use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, get_service, post, put};
use axum::{Extension, Router};
use serde::Serialize;
use tokio::sync::oneshot;
use tower_cookies::CookieManagerLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::info;
use ts_rs::TS;
use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};
use utoipa::{Modify, OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use xtra::WeakAddress;

use crate::SqlitePool;
use crate::bot::MasterBot;
use crate::db_util::{schema_opt_duration, serialize_opt_duration};
use crate::web_server::api::{album, audio_file, favourite, song};
use crate::youtube_dl::AudioMetadata;

mod api;
mod bot_data;
mod login;
pub use bot_data::*;

#[derive(OpenApi)]
#[openapi(
    paths(
        login::token,
        audio_file::upload,
        audio_file::get_all,
        song::get_songs,
        album::get_albums,
        song::put_metadata,
        api::get_bot,
        api::put_state,
        api::get_currently_playing,
        api::post_currently_playing,
        favourite::get_all,
        favourite::post,
        favourite::delete,
    ),
    components(schemas(
        login::Login,
        audio_file::AudioFile,
        audio_file::SongMetadata,
        audio_file::UploadResponse,
        song::Song,
        favourite::Favourite,
    )),
    modifiers(&SecurityAddon),
    security(
        ("pokebot-token-header" = []),
    ),
)]
struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "pokebot-token-header",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}

pub async fn start(
    bind_address: String,
    bot: WeakAddress<MasterBot>,
    db_pool: SqlitePool,
    shutdown_rx: oneshot::Receiver<()>,
) -> std::io::Result<()> {
    info!("Listening on {}", &bind_address);
    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    axum::serve(
        listener,
        Router::new()
            .merge(SwaggerUi::new("/swagger").url("/api-doc/openapi.json", ApiDoc::openapi()))
            .route("/api/login", post(login::token))
            .route("/api/audio", get(audio_file::get_all))
            .route(
                "/api/audio",
                post(audio_file::upload).layer(DefaultBodyLimit::max(100 * 1024 * 1024)),
            )
            .route("/api/audio/{id}/metadata", put(song::put_metadata))
            .route("/api/song", get(song::get_songs))
            .route("/api/album", get(album::get_albums))
            .route("/api/favourite", get(favourite::get_all))
            .route("/api/song/{id}/favourite", post(favourite::post))
            .route("/api/song/{id}/favourite", delete(favourite::delete))
            .route("/api/playlist/current", get(api::get_currently_playing))
            .route("/api/playlist/current", post(api::post_currently_playing))
            .route("/api/bot/self", get(api::get_bot))
            .route("/api/bot/self", put(api::put_state))
            .nest_service("/covers", get_service(ServeDir::new("./covers")))
            .layer(Extension(db_pool))
            .layer(CorsLayer::permissive())
            .layer(TraceLayer::new_for_http())
            .layer(Extension(bot.clone()))
            .layer(CookieManagerLayer::new()),
    )
    .with_graceful_shutdown(async {
        shutdown_rx.await.unwrap();
    })
    .await?;

    Ok(())
}

#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "../web_server-types/")]
pub struct BotData {
    pub name: String,

    pub state: crate::bot::State,

    pub volume: f64,

    #[serde(serialize_with = "serialize_opt_duration")]
    #[schema(schema_with = schema_opt_duration)]
    #[ts(type = "number | null")]
    pub position: Option<Duration>,

    pub currently_playing: Option<AudioMetadata>,

    pub playlist: Vec<AudioMetadata>,
}
