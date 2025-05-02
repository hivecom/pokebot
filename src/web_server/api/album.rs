use anyhow::Context;
use axum::{Extension, Json};
use diesel::prelude::Queryable;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl};
use diesel_async::RunQueryDsl;
use serde::Serialize;
use ts_rs::TS;
use utoipa::ToSchema;

use crate::db_util::unix_timestamp;
use crate::schema::{albums, artists};
use crate::web_server::api::Error;
use crate::web_server::api::song::{Song, songs};
use crate::{SqliteConn, SqlitePool};

const DEFAULT_ARTIST: &str = "Unknown Artist";

/// The definition of an album.
#[derive(Debug, Serialize, TS, ToSchema, Queryable)]
#[ts(export, export_to = "../web_server-types/")]
pub struct AlbumMetadata {
    #[schema(example = 1)]
    #[ts(type = "number")]
    pub id: i64,

    /// The name of the song
    #[schema(example = "Album name")]
    pub title: String,

    /// The creator of the song
    #[schema(example = "Artist")]
    pub artist: String,

    pub cover: Option<String>,

    /// A unix timestamp of when this song was added
    #[schema(example = 1670802822)]
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Debug, Serialize, TS, ToSchema)]
#[ts(export, export_to = "../web_server-types/")]
pub struct Album {
    #[serde(flatten)]
    pub metadata: AlbumMetadata,

    pub songs: Vec<Song>,
}

pub enum AlbumFilter {
    Id(i64),
    Albumless,
    Any,
}

pub async fn albums(conn: &mut SqliteConn) -> Result<Vec<Album>, Error> {
    let mut albums = Vec::new();

    for metadata in albums::table
        .inner_join(artists::table)
        .select((
            albums::id,
            albums::title,
            artists::name,
            albums::cover,
            albums::created_at,
        ))
        .get_results::<AlbumMetadata>(conn)
        .await
        .context("Failed to get albums")?
    {
        let mut songs = songs(conn, AlbumFilter::Id(metadata.id)).await?;
        songs.sort();
        albums.push(Album { songs, metadata })
    }
    let albumless_songs = songs(conn, AlbumFilter::Albumless).await?;
    if !albumless_songs.is_empty() {
        albums.push(Album {
            metadata: AlbumMetadata {
                id: -1,
                title: String::from("No Album"),
                artist: String::from(DEFAULT_ARTIST),
                cover: None,
                created_at: 0,
            },
            songs: albumless_songs,
        });
    }

    Ok(albums)
}

/// Get albums
#[utoipa::path(
    get,
    path = "/api/album",
    responses(
        (status = 200, description = "Got all albums", body = [Album])
    )
)]
pub async fn get_albums(Extension(pool): Extension<SqlitePool>) -> Result<Json<Vec<Album>>, Error> {
    let mut conn = pool.get().await.expect("can connect to sqlite");

    Ok(Json(albums(&mut conn).await?))
}

pub async fn get_or_insert_album(
    conn: &mut SqliteConn,
    uid: &str,
    artist_id: i64,
    cover_path: &Option<String>,
    album: &Option<String>,
) -> Result<Option<(i64, String)>, Error> {
    let res = if let Some(album) = album {
        let res = diesel::insert_into(albums::table)
            .values((
                albums::title.eq(&album),
                albums::artist_id.eq(&artist_id),
                albums::cover.eq(cover_path),
                albums::created_at.eq(unix_timestamp()),
                albums::created_by.eq(uid),
            ))
            .on_conflict(albums::title)
            .do_nothing()
            .returning((albums::id, albums::title))
            .get_result(conn)
            .await
            .optional()
            .context("Failed to insert album")?;

        let res = match res {
            Some(res) => res,
            None => albums::table
                .filter(albums::title.eq(album))
                .select((albums::id, albums::title))
                .get_result::<(i64, String)>(conn)
                .await
                .context("Failed to get album")?,
        };

        Some(res)
    } else {
        None
    };

    Ok(res)
}
