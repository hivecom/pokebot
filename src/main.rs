use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::Context;
use diesel::Connection;
use diesel::connection::Instrumentation;
use diesel_async::pooled_connection::{AsyncDieselConnectionManager, ManagerConfig, deadpool};
use diesel_async::sync_connection_wrapper::SyncConnectionWrapper;
use diesel_async::{AsyncMigrationHarness, RunQueryDsl};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use structopt::StructOpt;
use structopt::clap::AppSettings;
#[cfg(unix)]
use tokio::signal::unix::*;
use tokio::sync::oneshot;
use tracing::level_filters::LevelFilter;
use tracing::{Level, span, trace};
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;
use tsclientlib::Identity;

mod audio_player;
mod bot;
mod command;
mod db_util;
mod playlist;
mod schema;
mod sqlite_mapping;
mod teamspeak;
mod web_server;
mod youtube_dl;

use bot::{MasterArgs, MasterBot, Quit};

#[derive(StructOpt, Debug)]
#[structopt(global_settings = &[AppSettings::ColoredHelp])]
pub struct Args {
    #[structopt(short = "l", long = "local", help = "Run locally in text mode")]
    local: bool,
    #[structopt(
        short = "g",
        long = "generate-identities",
        help = "Generate 'count' identities"
    )]
    gen_id_count: Option<u8>,
    /// Increases the security level of all identities in the config file
    #[structopt(short, long = "increase-security-level")]
    wanted_level: Option<u8>,
    #[structopt(
        short = "a",
        long = "address",
        help = "The address of the server to connect to"
    )]
    address: Option<String>,
    #[structopt(
        help = "Configuration file",
        parse(from_os_str),
        default_value = "config.toml"
    )]
    config_path: PathBuf,
    #[structopt(
        short = "d",
        long = "master_channel",
        help = "The channel the master bot should connect to"
    )]
    master_channel: Option<String>,
    // 0. Print nothing
    // 1. Print command string
    // 2. Print packets
    // 3. Print udp packets
    #[structopt(
        short = "v",
        long = "verbose",
        help = "Print the content of all packets",
        parse(from_occurrences)
    )]
    verbose: u8,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    tracing_subscriber::fmt().with_env_filter(filter).init();

    if let Err(e) = run().await {
        error!("{}", e);
    }
}

async fn run() -> Result<(), anyhow::Error> {
    // Parse command line options
    let args = Args::from_args();

    // Set up signal handlers
    let ctrl_c = tokio::task::spawn(tokio::signal::ctrl_c());
    #[cfg(unix)]
    let (sighup, sigterm, sigquit) = (
        tokio::task::spawn(hangup()),
        tokio::task::spawn(terminate()),
        tokio::task::spawn(quit()),
    );

    let mut file = File::open(&args.config_path)
        .with_context(|| format!("Failed to open {}", args.config_path.to_string_lossy()))?;
    let mut toml = String::new();
    file.read_to_string(&mut toml)?;

    let mut config: MasterArgs = toml::from_str(&toml)?;

    if let Some(music_root) = &config.music_root
        && !music_root.is_dir()
    {
        anyhow::bail!("music_root is not a directory");
    }

    if config.id.is_none() {
        config.id = Some(Identity::create());
    }

    if let Some(count) = args.gen_id_count {
        for _ in 0..count {
            let id = Identity::create();
            if let Some(ids) = &mut config.ids {
                ids.push(id);
            } else {
                config.ids = Some(vec![id]);
            }
        }

        let toml = toml::to_string(&config)?;
        let mut file = File::create(&args.config_path)?;
        file.write_all(toml.as_bytes())?;

        return Ok(());
    }

    if let Some(level) = args.wanted_level {
        if let Some(id) = &mut config.id {
            info!("Upgrading master identity");
            id.upgrade_level(level);
        }

        if let Some(ids) = &mut config.ids {
            let len = ids.len();
            for (i, id) in ids.iter_mut().enumerate() {
                info!("current" = i + 1, "amount" = len, "Upgrading bot identity");
                id.upgrade_level(level);
            }
        }

        let toml = toml::to_string(&config)?;
        let mut file = File::create(&args.config_path)?;
        file.write_all(toml.as_bytes())?;

        return Ok(());
    }

    if config.id.is_none() || config.ids.is_none() {
        error!("Failed to find required identites, try running with `-g`");
        return Ok(());
    }

    let bot_args = config.merge(args);

    info!("Starting PokeBot!");
    debug!(args = ?std::env::args(), "Received CLI arguments");

    let bot_name = bot_args.master_name.clone();
    let webserver_enable = bot_args.webserver_enable;
    let bind_address = bot_args.bind_address.clone();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let db_path = std::env::var("DATABASE_URL").unwrap();
    let db_pool = setup_database(&db_path).await.unwrap();

    let bot = MasterBot::spawn(
        bot_args,
        db_pool.clone(),
        span!(Level::ERROR, "", master = bot_name.clone()),
    )
    .await;

    if webserver_enable {
        let bot = bot.downgrade();
        tokio::spawn(async move {
            if let Err(error) = web_server::start(bind_address, bot, db_pool, shutdown_rx).await {
                error!(%error, "Error in web server");
            }
        });
    }

    #[cfg(unix)]
    tokio::select! {
        res = ctrl_c => {
            res??;
            info!(signal = "SIGINT", "Received signal, shutting down");
        }
        _ = sigterm => {
            info!(signal = "SIGTERM", "Received signal, shutting down");
        }
        _ = sighup => {
            info!(signal = "SIGHUP", "Received signal, shutting down");
        }
        _ = sigquit => {
            info!(signal = "SIGQUIT", "Received signal, shutting down");
        }
    };

    #[cfg(windows)]
    ctrl_c.await??;

    shutdown_tx.send(()).unwrap();

    bot.send(Quit(String::from("Stopping")))
        .await
        .unwrap()
        .unwrap();

    Ok(())
}

#[cfg(unix)]
pub async fn terminate() -> std::io::Result<()> {
    signal(SignalKind::terminate())?.recv().await;
    Ok(())
}

#[cfg(unix)]
pub async fn hangup() -> std::io::Result<()> {
    signal(SignalKind::hangup())?.recv().await;
    Ok(())
}

#[cfg(unix)]
pub async fn quit() -> std::io::Result<()> {
    signal(SignalKind::quit())?.recv().await;
    Ok(())
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

type SqliteAsyncConn = SyncConnectionWrapper<diesel::SqliteConnection>;
type SqlitePool = deadpool::Pool<SqliteAsyncConn>;
type SqliteConn = deadpool::Object<SqliteAsyncConn>;

pub async fn setup_database(database_url: &str) -> anyhow::Result<SqlitePool> {
    let mut config = ManagerConfig::<SqliteAsyncConn>::default();

    config.custom_setup = Box::new(|url| {
        Box::pin(async move {
            let mut sync_conn = diesel::SqliteConnection::establish(url)?;
            let mut conn = SyncConnectionWrapper::new(sync_conn);

            diesel::sql_query("PRAGMA foreign_keys = ON")
                .execute(&mut conn)
                .await
                .map_err(|e| diesel::result::ConnectionError::BadConnection(e.to_string()))?;

            diesel::sql_query("PRAGMA busy_timeout = 1000")
                .execute(&mut conn)
                .await
                .map_err(|e| diesel::result::ConnectionError::BadConnection(e.to_string()))?;

            Ok(conn)
        })
    });

    let manager =
        AsyncDieselConnectionManager::<SqliteAsyncConn>::new_with_config(database_url, config);

    let pool = deadpool::Pool::builder(manager).build()?;
    let connection = pool.get().await.expect("can connect to sqlite");

    let mut harness = AsyncMigrationHarness::new(connection);
    harness
        .run_pending_migrations(MIGRATIONS)
        .expect("migrations should be tested to work without error");

    Ok(pool)
}
