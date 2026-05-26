use std::time::Duration;

use anyhow::Context;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl};
use diesel_async::RunQueryDsl;
use serde::Deserialize;
use ts_rs::TS;
use utoipa::ToSchema;
use xtra::WeakAddress;

use crate::command::{Command, Seek, VolumeChange};
use crate::db_util::{deserialize_opt_duration, schema_opt_duration};
use crate::schema::{audio_files, songs};
use crate::web_server::{BotData, BotDataRequest, CommandRequest, ConfigVars, CreateBotRequest};
use crate::youtube_dl::AudioMetadata;
use crate::{MasterBot, SqlitePool};

use super::login::TsToken;
use error::Error;

pub mod album;
pub mod audio_file;
pub mod error;
pub mod favourite;
pub mod song;

/// Get current playlist
#[utoipa::path(
    get,
    path = "/api/playlist/current",
    responses(
        (status = 200, description = "Got currently playing", body = [AudioMetadata])
    )
)]
pub async fn get_currently_playing(
    Extension(bot): Extension<WeakAddress<MasterBot>>,
    TsToken(token): TsToken,
) -> Result<Json<Vec<AudioMetadata>>, Error> {
    if let Some(mut bot_data) = bot.send(BotDataRequest { token }).await.unwrap() {
        bot_data
            .playlist
            .iter_mut()
            .for_each(|m| m.thumbnail = None);
        Ok(Json(bot_data.playlist))
    } else {
        Err(Error::NotFound)
    }
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export, export_to = "../web_server-types/")]
pub struct PlaySong {
    #[ts(type = "number")]
    pub id: i64,
    // TODO: allow any playlist position
}

/// Post new song to currently playing
#[utoipa::path(
    post,
    path = "/api/playlist/current",
    responses(
        (status = 201, description = "Got currently playing", body = AudioMetadata)
    )
)]
pub async fn post_currently_playing(
    Extension(pool): Extension<SqlitePool>,
    Extension(bot): Extension<WeakAddress<MasterBot>>,
    Extension(vars): Extension<ConfigVars>,
    TsToken(token): TsToken,
    req: Result<Json<PlaySong>, JsonRejection>,
) -> Result<(StatusCode, Json<AudioMetadata>), Error> {
    let mut conn = pool.get().await.expect("can connect to sqlite");

    let Json(PlaySong { id }) = req?;

    let file_path: String = audio_files::table
        .inner_join(songs::table)
        .filter(songs::id.eq(id))
        .select(audio_files::file_path)
        .get_result(&mut conn)
        .await
        .optional()
        .context("Failed to get song file path")?
        .ok_or(Error::NotFound)?;

    let mut absolute = vars.music_root;
    absolute.push(file_path);
    let url = absolute.to_str().unwrap();

    bot.send(CommandRequest {
        token: token.clone(),
        command: Command::Add {
            url: vec![format!("file://{url}")],
        },
    })
    .await
    .unwrap()?;

    let bot_data = bot
        .send(BotDataRequest { token })
        .await
        .unwrap()
        .context("Failed to get bot data")?;

    Ok((
        StatusCode::CREATED,
        Json(
            bot_data
                .playlist
                .last()
                .cloned()
                .unwrap_or(bot_data.currently_playing.unwrap()),
        ),
    ))
}

/// Get bot data
#[utoipa::path(
    get,
    path = "/api/bot/self",
    responses(
        (status = 200, description = "Got bot data", body = BotData)
    )
)]
pub async fn get_bot(
    Extension(bot): Extension<WeakAddress<MasterBot>>,
    TsToken(token): TsToken,
) -> impl IntoResponse {
    if let Some(bot_data) = bot.send(BotDataRequest { token }).await.unwrap() {
        Ok(Json(bot_data))
    } else {
        Err(Error::NotFound)
    }
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export, export_to = "../web_server-types/")]
pub struct PutState {
    pub playing: Option<bool>,

    pub volume: Option<f64>,

    #[schema(example = 170)]
    #[serde(deserialize_with = "deserialize_opt_duration")]
    #[schema(schema_with = schema_opt_duration)]
    #[ts(type = "number | null")]
    pub seek: Option<Duration>,

    #[serde(default)]
    pub next: bool,
}

/// Add a bot to users channel
#[utoipa::path(
    put,
    path = "/api/bot/self",
    responses(
        (status = 200, description = "Updated bot state", body = BotData)
    )
)]
pub async fn post_bot(
    Extension(bot): Extension<WeakAddress<MasterBot>>,
    TsToken(token): TsToken,
) -> Result<Json<BotData>, Error> {
    let bot_data = bot
        .send(CreateBotRequest { token })
        .await
        .unwrap()
        .context("Failed to get bot data")?;

    Ok(Json(bot_data))
}

/// Put new bot state update
#[utoipa::path(
    put,
    path = "/api/bot/self",
    responses(
        (status = 200, description = "Updated bot state", body = BotData)
    )
)]
pub async fn put_state(
    Extension(bot): Extension<WeakAddress<MasterBot>>,
    TsToken(token): TsToken,
    req: Result<Json<PutState>, JsonRejection>,
) -> Result<Json<BotData>, Error> {
    let Json(PutState {
        playing,
        volume,
        seek,
        next,
    }) = req?;

    if let Some(playing) = playing {
        bot.send(CommandRequest {
            token: token.clone(),
            command: if playing {
                Command::Play
            } else {
                Command::Pause
            },
        })
        .await
        .unwrap()?;
    };
    if let Some(volume) = volume {
        bot.send(CommandRequest {
            token: token.clone(),
            command: Command::Volume {
                volume: VolumeChange::Absolute(volume),
            },
        })
        .await
        .unwrap()?;
    };
    if let Some(seek) = seek {
        bot.send(CommandRequest {
            token: token.clone(),
            command: Command::Seek {
                amount: Seek::Absolute(seek),
            },
        })
        .await
        .unwrap()?;
    };

    if next {
        bot.send(CommandRequest {
            token: token.clone(),
            command: Command::Next,
        })
        .await
        .unwrap()?;
    }

    let bot_data = bot
        .send(BotDataRequest { token })
        .await
        .unwrap()
        .context("Failed to get bot data")?;

    Ok(Json(bot_data))
}
