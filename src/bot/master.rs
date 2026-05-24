use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context as AnyhowContext;
use async_trait::async_trait;
use diesel::ExpressionMethods;
use diesel::query_dsl::methods::{FilterDsl, SelectDsl};
use diesel_async::RunQueryDsl;
use futures::future;
use petname::Generator;
use rand::{SeedableRng, rngs::SmallRng, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use tracing::{Level, Span, debug, error, info, span, trace};
use tsclientlib::data::Client;
use tsclientlib::{
    ChannelGroupId, ChannelId, ClientDbId, ClientId, ClientType, ConnectOptions, Connection,
    IconId, Identity, MessageTarget,
};

use xtra::{Actor, Address, Context, Handler, Message, WeakAddress, spawn::Tokio};

use crate::schema::tokens;
use crate::teamspeak::TeamSpeakConnection;

use crate::db_util::unix_timestamp;
use crate::{Args, SqliteConn, SqlitePool};

use crate::bot::{GetBotData, GetChannel, GetName, MusicBot, MusicBotArgs, MusicBotMessage};

pub struct MasterBot {
    config: MasterConfig,
    my_addr: Option<WeakAddress<Self>>,
    teamspeak: TeamSpeakConnection,
    available_names: Vec<String>,
    available_ids: Vec<Identity>,
    connected_bots: HashMap<String, Address<MusicBot>>,
    rng: SmallRng,
    db_pool: SqlitePool,
    span: Span,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MasterArgs {
    #[serde(default = "default_name")]
    pub master_name: String,
    pub music_root: Option<PathBuf>,
    #[serde(default = "default_local")]
    pub local: bool,
    pub address: String,
    pub channel: Option<String>,
    pub volume: f64,
    #[serde(default = "default_verbose")]
    pub verbose: u8,
    pub bind_address: String,
    pub webserver_enable: bool,
    pub names: Vec<String>,
    pub id: Option<Identity>,
    pub ids: Option<Vec<Identity>>,
}

impl MasterBot {
    pub async fn spawn(args: MasterArgs, db_pool: SqlitePool, span: Span) -> Address<Self> {
        let config = MasterConfig {
            master_name: args.master_name,
            music_root: args.music_root,
            local: args.local,
            address: args.address,
            verbose: args.verbose,
            volume: args.volume,
        };

        if config.local {
            info!(parent: &span, "Starting in local mode");

            let mut conn = db_pool.get().await.expect("can connect to sqlite");
            if let Err(e) = insert_web_token(&mut conn, "local", "local").await {
                debug!("{e}");
            }

            let bot_addr = Self {
                config,
                my_addr: None,
                teamspeak: TeamSpeakConnection::new(span.clone()).await.unwrap(),
                rng: SmallRng::from_entropy(),
                available_names: args.names,
                available_ids: args.ids.expect("identities"),
                connected_bots: HashMap::new(),
                db_pool,
                span: span.clone(),
            }
            .create(None)
            .spawn(&mut Tokio::Global);
            trace!(parent: &span, "Spawned master bot actor");

            bot_addr
        } else {
            info!(parent: &span, "Starting in TeamSpeak mode");

            let mut con_config = Connection::build(config.address.clone())
                .version(tsclientlib::Version::Linux_3_3_2)
                .name(config.master_name.clone())
                .identity(args.id.expect("identity should exist"))
                .log_commands(args.verbose >= 1)
                .log_packets(args.verbose >= 2)
                .log_udp_packets(args.verbose >= 3);

            if let Some(channel) = args.channel {
                con_config = con_config.channel(channel);
            }

            let connection = TeamSpeakConnection::new(span.clone()).await.unwrap();
            trace!(parent: &span, "Created teamspeak connection");

            let bot_addr = Self {
                config,
                my_addr: None,
                teamspeak: connection,
                rng: SmallRng::from_entropy(),
                available_names: args.names,
                available_ids: args.ids.expect("identities"),
                connected_bots: HashMap::new(),
                db_pool,
                span: span.clone(),
            }
            .create(None)
            .spawn(&mut Tokio::Global);

            bot_addr.send(Connect(con_config)).await.unwrap().unwrap();
            trace!(parent: &span, "Spawned master bot actor");

            bot_addr
        }
    }

    async fn bot_args_for_client(
        &mut self,
        user_id: ClientId,
    ) -> std::result::Result<MusicBotArgs, BotCreationError> {
        let channel = match self.teamspeak.channel_of_user(user_id).await.unwrap() {
            Some(channel) => channel,
            None => return Err(BotCreationError::UnfoundUser),
        };

        if Some(channel) == self.teamspeak.current_channel().await {
            return Err(BotCreationError::MasterChannel(
                self.config.master_name.clone(),
            ));
        }

        for bot in self.connected_bots.values() {
            if let Ok(c) = bot.send(GetChannel).await
                && c == Some(channel)
            {
                return Err(BotCreationError::MultipleBots(
                    bot.send(GetName).await.unwrap(),
                ));
            }
        }

        let channel_path = self
            .teamspeak
            .channel_path_of_user(user_id)
            .await
            .expect("can find poke sender")
            .expect("can find poke sender");

        self.available_names.shuffle(&mut self.rng);
        let name = match self.available_names.pop() {
            Some(v) => v,
            None => {
                return Err(BotCreationError::OutOfNames);
            }
        };

        self.available_ids.shuffle(&mut self.rng);
        let identity = match self.available_ids.pop() {
            Some(v) => v,
            None => {
                return Err(BotCreationError::OutOfIdentities);
            }
        };

        Ok(MusicBotArgs {
            name: name.clone(),
            music_root: self.config.music_root.clone(),
            master: self.my_addr.clone(),
            address: self.config.address.clone(),
            identity,
            local: false,
            channel: channel_path,
            verbose: self.config.verbose,
            span: span!(parent: &self.span, Level::ERROR, "", name),
            volume: self.config.volume,
        })
    }

    async fn spawn_bot_for_client(&mut self, id: ClientId) -> anyhow::Result<()> {
        match self.bot_args_for_client(id).await {
            Ok(bot_args) => {
                let name = bot_args.name.clone();
                let bot = MusicBot::spawn(bot_args).await;
                self.connected_bots.insert(name, bot);
            }
            Err(e) => {
                self.teamspeak
                    .send_message_to_user(id, e.to_string())
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn uid_by_web_token(&self, token: &str) -> Result<String, anyhow::Error> {
        if self.config.local {
            return Ok(String::from("local"));
        }

        let mut conn = self.db_pool.get().await.expect("can connect to sqlite");

        let uid = tokens::table
            .filter(tokens::token.eq(token))
            .select(tokens::uid)
            .get_result(&mut conn)
            .await
            .context("Faielld to get token by uid")?;

        Ok(uid)
    }

    async fn on_message(&mut self, message: MusicBotMessage) -> anyhow::Result<()> {
        match message {
            MusicBotMessage::TextMessage(message) => match message.target {
                MessageTarget::Poke(user) => {
                    info!(
                        parent: &self.span,
                        %user,
                        "Poked, creating bot"
                    );
                    self.spawn_bot_for_client(user).await?;
                }
                MessageTarget::Client(_) => {
                    if message.text == "auth" {
                        let token = petname::Petnames::default()
                            .generate_one(5, "-")
                            .expect("no names");
                        let uid = message.invoker.uid.unwrap();

                        let mut conn = self.db_pool.get().await.expect("can connect to sqlite");
                        insert_web_token(&mut conn, &token, &uid.to_string()).await?;
                        self.teamspeak
                            .send_message_to_user(message.invoker.id, token)
                            .await?;
                    }
                }
                _ => (),
            },
            MusicBotMessage::ClientAdded(id) => {
                if id == self.teamspeak.my_id().await? {
                    self.teamspeak
                        .set_description(String::from("Poke me if you want a music bot!"))
                        .await;
                }
            }
            _ => (),
        }

        Ok(())
    }

    pub async fn bot_data(&mut self, token: &str) -> Option<crate::web_server::BotData> {
        let bot = self.bot_by_user_token(token).await?;

        bot.send(GetBotData).await.ok()
    }

    pub async fn bot_datas(&self) -> Vec<crate::web_server::BotData> {
        let len = self.connected_bots.len();
        let mut result = Vec::with_capacity(len);
        for bot in self.connected_bots.values() {
            let bot_data = bot.send(GetBotData).await.unwrap();
            result.push(bot_data);
        }

        result
    }

    pub fn bot_names(&self) -> Vec<String> {
        let len = self.connected_bots.len();
        let mut result = Vec::with_capacity(len);
        for name in self.connected_bots.keys() {
            result.push(name.clone());
        }

        result
    }

    pub async fn client_by_user_token(&mut self, token: &str) -> Option<Client> {
        if self.config.local {
            return Some(default_client());
        }

        let uid = self.uid_by_web_token(token).await.ok()?;
        self.teamspeak.user_by_uid(uid.clone()).await.unwrap()
    }

    pub async fn bot_by_channel(&mut self, channel: ChannelId) -> Option<WeakAddress<MusicBot>> {
        if self.config.local {
            return self.connected_bots.values().nth(0).map(|b| b.downgrade());
        }

        for bot in self.connected_bots.values() {
            if Some(channel) == bot.send(GetChannel).await.unwrap() {
                return Some(bot.downgrade());
            }
        }

        None
    }

    pub async fn bot_by_user_token(&mut self, token: &str) -> Option<WeakAddress<MusicBot>> {
        // TODO: this should be handled by bot_by_channel once client_by_user_token is adjusted
        if self.config.local {
            return self.connected_bots.values().nth(0).map(|b| b.downgrade());
        }

        if let Some(client) = self.client_by_user_token(token).await {
            return self.bot_by_channel(client.channel).await;
        }

        None
    }

    fn on_bot_disconnect(&mut self, name: String, id: Identity) {
        self.connected_bots.remove(&name);
        self.available_names.push(name);
        self.available_ids.push(id);
    }

    pub async fn quit(&mut self, reason: String) -> anyhow::Result<()> {
        let futures = self
            .connected_bots
            .values()
            .map(|b| b.send(Quit(reason.clone())));
        for res in future::join_all(futures).await {
            if let Err(error) = res {
                error!(parent: &self.span, %error, "Failed to shut down bot");
            }
        }
        self.teamspeak.disconnect(&reason).await
    }
}

async fn insert_web_token(conn: &mut SqliteConn, token: &str, uid: &str) -> anyhow::Result<()> {
    diesel::insert_into(tokens::table)
        .values((
            tokens::token.eq(token),
            tokens::uid.eq(uid),
            tokens::created_at.eq(unix_timestamp()),
        ))
        .execute(conn)
        .await
        .context("Failed to insert token for uid")?;

    Ok(())
}

#[async_trait]
impl Actor for MasterBot {
    async fn started(&mut self, ctx: &mut Context<Self>) {
        self.my_addr = Some(ctx.address().unwrap().downgrade());

        if self.config.local {
            let name = self.available_names[0].clone();
            let bot_args = MusicBotArgs {
                name: name.clone(),
                music_root: self.config.music_root.clone(),
                master: self.my_addr.clone(),
                local: true,
                address: self.config.address.clone(),
                identity: self.available_ids[0].clone(),
                channel: String::from("local"),
                verbose: self.config.verbose,
                volume: self.config.volume,
                span: span!(Level::ERROR, "", name),
            };
            let bot = MusicBot::spawn(bot_args).await;
            self.connected_bots.insert(name, bot);
        }
    }
}

pub struct Connect(pub ConnectOptions);
impl Message for Connect {
    type Result = anyhow::Result<()>;
}

#[async_trait]
impl Handler<Connect> for MasterBot {
    async fn handle(&mut self, opt: Connect, ctx: &mut Context<Self>) -> anyhow::Result<()> {
        let addr = ctx.address().unwrap();
        self.teamspeak.connect_for_bot(opt.0, addr.downgrade())?;
        Ok(())
    }
}

pub struct Quit(pub String);
impl Message for Quit {
    type Result = anyhow::Result<()>;
}

#[async_trait]
impl Handler<Quit> for MasterBot {
    async fn handle(&mut self, q: Quit, _: &mut Context<Self>) -> anyhow::Result<()> {
        self.quit(q.0).await
    }
}

pub struct BotDisonnected {
    pub name: String,
    pub identity: Identity,
}

impl Message for BotDisonnected {
    type Result = ();
}

#[async_trait]
impl Handler<BotDisonnected> for MasterBot {
    async fn handle(&mut self, dc: BotDisonnected, _: &mut Context<Self>) {
        self.on_bot_disconnect(dc.name, dc.identity);
    }
}

#[async_trait]
impl Handler<MusicBotMessage> for MasterBot {
    async fn handle(&mut self, msg: MusicBotMessage, _: &mut Context<Self>) -> anyhow::Result<()> {
        self.on_message(msg).await
    }
}

#[derive(Debug)]
pub enum BotCreationError {
    UnfoundUser,
    MasterChannel(String),
    MultipleBots(String),
    OutOfNames,
    OutOfIdentities,
}

impl std::fmt::Display for BotCreationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use BotCreationError::*;
        match self {
            UnfoundUser => write!(
                f,
                "I can't find you in the channel list, \
                    either I am not subscribed to your channel or this is a bug.",
            ),
            MasterChannel(name) => write!(f, "Joining the channel of \"{}\" is not allowed", name),
            MultipleBots(name) => write!(
                f,
                "\"{}\" is already in this channel. \
                         Multiple bots in one channel are not allowed.",
                name
            ),
            OutOfNames => write!(f, "Out of names. Too many bots are already connected!"),

            OutOfIdentities => write!(f, "Out of identities. Too many bots are already connected!"),
        }
    }
}

fn default_name() -> String {
    String::from("PokeBot")
}

fn default_verbose() -> u8 {
    0
}

fn default_local() -> bool {
    false
}

impl MasterArgs {
    pub fn merge(self, args: Args) -> Self {
        let local = args.local || self.local;
        let address = args.address.unwrap_or(self.address);
        let channel = args.master_channel.or(self.channel);
        let verbose = if args.verbose > 0 {
            args.verbose
        } else {
            self.verbose
        };

        Self {
            master_name: self.master_name,
            music_root: self.music_root,
            names: self.names,
            ids: self.ids,
            local,
            address,
            bind_address: self.bind_address,
            webserver_enable: self.webserver_enable,
            id: self.id,
            channel,
            verbose,
            volume: self.volume,
        }
    }
}

pub struct MasterConfig {
    pub master_name: String,
    pub music_root: Option<PathBuf>,
    pub local: bool,
    pub address: String,
    pub verbose: u8,
    pub volume: f64,
}

fn default_client() -> Client {
    Client {
        id: ClientId(0),
        name: Default::default(),
        channel: ChannelId(0),
        channel_group: ChannelGroupId(0),
        client_type: ClientType::Normal,
        avatar_hash: Default::default(),
        away_message: Default::default(),
        badges: Default::default(),
        connection_data: Default::default(),
        country_code: Default::default(),
        database_id: ClientDbId(0),
        description: Default::default(),
        icon: IconId(0),
        inherited_channel_group_from_channel: ChannelId(0),
        input_hardware_enabled: Default::default(),
        input_muted: Default::default(),
        is_channel_commander: Default::default(),
        is_priority_speaker: Default::default(),
        is_recording: Default::default(),
        metadata: Default::default(),
        needed_serverquery_view_power: Default::default(),
        optional_data: Default::default(),
        output_hardware_enabled: Default::default(),
        output_muted: Default::default(),
        output_only_muted: Default::default(),
        permission_hints: Default::default(),
        phonetic_name: Default::default(),
        server_groups: Default::default(),
        talk_power: Default::default(),
        talk_power_granted: Default::default(),
        talk_power_request: Default::default(),
        uid: Default::default(),
        unread_messages: Default::default(),
        user_tag: Default::default(),
    }
}
