use std::cmp::Ordering;

use anyhow::Context;
use axum::extract::Path;
use axum::extract::rejection::JsonRejection;
use axum::{Extension, Json};
use diesel::dsl::sql;
use diesel::prelude::{Insertable, Queryable, QueryableByName};
use diesel::{
    ExpressionMethods, JoinOnDsl, NullableExpressionMethods, OptionalExtension, QueryDsl,
};
use diesel_async::scoped_futures::ScopedFutureExt;
use diesel_async::{AsyncConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::ToSchema;

use crate::db_util::unix_timestamp;
use crate::schema::favourites;
use crate::schema::{albums, artists, songs};
use crate::web_server::api::Error;
use crate::web_server::api::album::{AlbumFilter, get_or_insert_album};
use crate::web_server::login::{TsToken, uid_by_token};
use crate::{SqliteConn, SqlitePool};

/// The definition of a song.
#[derive(Debug, Serialize, TS, ToSchema, Queryable, Eq)]
#[ts(export, export_to = "../web_server-types/")]
pub struct Song {
    #[schema(example = 1)]
    #[ts(type = "number")]
    pub id: i64,

    /// The id of the song within the album
    #[schema(example = 1)]
    #[ts(type = "number | null")]
    pub track: Option<i64>,

    /// The name of the song
    #[schema(example = "Song name")]
    pub title: String,

    /// The creator of the song
    #[schema(example = "Artist")]
    pub artist: String,

    /// The creator of the song
    #[schema(example = "Album")]
    pub album: Option<String>,

    /// The disc of the album
    #[ts(type = "number | null")]
    pub disc_number: Option<i64>,

    #[schema(example = 1)]
    #[ts(type = "number")]
    pub file_id: i64,

    #[ts(type = "number")]
    pub favourite_count: i64,

    /// A unix timestamp of when this song was added
    #[schema(example = 1670802822)]
    #[ts(type = "number")]
    pub created_at: i64,
}

impl PartialEq for Song {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialOrd for Song {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Song {
    fn cmp(&self, other: &Self) -> Ordering {
        self.track.cmp(&other.track)
    }
}

pub async fn songs(conn: &mut SqliteConn, album_filter: AlbumFilter) -> Result<Vec<Song>, Error> {
    let query = songs::table
        .inner_join(artists::table.on(songs::artist_id.eq(artists::id)))
        .left_join(albums::table.on(songs::album_id.eq(albums::id.nullable())))
        .left_join(favourites::table.on(songs::id.eq(favourites::song_id)))
        .group_by((
            songs::id,
            songs::track,
            songs::title,
            artists::name,
            albums::title, // Grouping by albums::title satisfies the nullability check
            songs::disc_number,
            songs::file_id,
            songs::created_at,
        ));
    // .group_by((songs::id, artists::name, albums::title));

    let select_clause = (
        songs::id,
        songs::track,
        songs::title,
        artists::name,
        albums::title.nullable(),
        songs::disc_number,
        songs::file_id,
        sql::<diesel::sql_types::BigInt>("COUNT(favourites.id)"),
        songs::created_at,
    );

    let songs = match album_filter {
        AlbumFilter::Id(album_id) => {
            query
                .filter(albums::id.eq(album_id))
                .select(select_clause)
                .get_results::<Song>(conn)
                .await
        }
        AlbumFilter::Albumless => {
            query
                .filter(albums::id.is_null())
                .select(select_clause)
                .get_results::<Song>(conn)
                .await
        }
        AlbumFilter::Any => query.select(select_clause).get_results::<Song>(conn).await,
    }
    .context("Failed to get songs")?;

    Ok(songs)
}

/// Get songs
#[utoipa::path(
    get,
    path = "/api/song",
    responses(
        (status = 200, description = "Got all songs", body = [Song])
    )
)]
pub async fn get_songs(Extension(pool): Extension<SqlitePool>) -> Result<Json<Vec<Song>>, Error> {
    let mut conn = pool.get().await.expect("can connect to sqlite");

    Ok(Json(songs(&mut conn, AlbumFilter::Any).await?))
}

#[derive(Debug, Deserialize, TS, ToSchema)]
#[ts(export, export_to = "../web_server-types/")]
pub struct PutSongMetadata {
    #[ts(type = "number | null")]
    pub track: Option<i64>,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    #[ts(type = "number | null")]
    pub disc_number: Option<i64>,
    pub cover_path: Option<String>,
}

/// Upsert song metadata
#[utoipa::path(
    put,
    path = "/api/audio/{id}/metadata",
    request_body(content = PutSongMetadata, description = "Metadata to set"),
    responses(
        (status = 200, description = "Put metadata", body = Song)
    )
)]
pub async fn put_metadata(
    Path(id): Path<i64>,
    Extension(pool): Extension<SqlitePool>,
    TsToken(token): TsToken,
    req: Result<Json<PutSongMetadata>, JsonRejection>,
) -> Result<Json<Song>, Error> {
    let mut conn = pool.get().await.expect("can connect to sqlite");
    let Json(metadata) = req?;

    let uid = uid_by_token(&mut conn, &token).await?;
    let song = update_song_metadata(&mut conn, &uid, id, metadata).await?;

    Ok(Json(song))
}

#[derive(Debug, ToSchema, Insertable)]
#[diesel(table_name = songs)]
pub struct DbSongMetadata {
    track: Option<i64>,
    title: String,
    artist_id: i64,
    album_id: Option<i64>,
    disc_number: Option<i64>,
    file_id: i64,
    created_at: i64,
    created_by: String,
}

pub async fn update_song_metadata(
    conn: &mut SqliteConn,
    uid: &str,
    file_id: i64,
    metadata: PutSongMetadata,
) -> Result<Song, Error> {
    let artist_id = get_or_insert_artist(conn, uid, &metadata.artist).await?;
    let album_id =
        get_or_insert_album(conn, uid, artist_id, &metadata.cover_path, &metadata.album).await?;

    let song = upsert_song(
        conn,
        &DbSongMetadata {
            track: metadata.track,
            title: metadata.title,
            artist_id,
            album_id,
            disc_number: metadata.disc_number,
            file_id,
            created_at: unix_timestamp(),
            created_by: uid.to_owned(),
        },
    )
    .await?;

    Ok(song)
}

pub async fn upsert_song(conn: &mut SqliteConn, metadata: &DbSongMetadata) -> Result<Song, Error> {
    diesel::insert_into(songs::table)
        .values(metadata)
        .on_conflict(songs::file_id)
        .do_update()
        .set((
            songs::track.eq(metadata.track),
            songs::title.eq(&metadata.title),
            songs::artist_id.eq(metadata.artist_id),
            songs::album_id.eq(metadata.album_id),
            songs::disc_number.eq(metadata.disc_number),
        ))
        .execute(conn) // Type <(i64, String)> is inferred
        .await
        .optional()
        .context("Failed to upsert song metadata")?;

    let song = songs::table
        .inner_join(artists::table.on(songs::artist_id.eq(artists::id)))
        .left_join(albums::table.on(songs::album_id.eq(albums::id.nullable())))
        .left_join(favourites::table.on(songs::id.eq(favourites::song_id)))
        .filter(songs::file_id.eq(metadata.file_id))
        .group_by((
            songs::id,
            songs::track,
            songs::title,
            artists::name,
            albums::title, // Grouping by albums::title satisfies the nullability check
            songs::disc_number,
            songs::file_id,
            songs::created_at,
        ))
        .select((
            songs::id,
            songs::track,
            songs::title,
            artists::name,
            albums::title.nullable(),
            songs::disc_number,
            songs::file_id,
            sql::<diesel::sql_types::BigInt>("COUNT(favourites.id)"),
            songs::created_at,
        ))
        .get_result::<Song>(conn)
        .await
        .context("Failed to get updated song")?;
    // .group_by((songs::id, artists::name, albums::title));

    Ok(song)
}

async fn get_or_insert_artist(
    conn: &mut SqliteConn,
    uid: &str,
    artist: &String,
) -> Result<i64, Error> {
    Ok(conn
        .transaction(|conn| {
            async {
                let existing = artists::table
                    .filter(artists::name.eq(artist))
                    .select(artists::id)
                    .get_result(conn) // Type <(i64, String)> is inferred
                    .await
                    .optional()
                    .context("Failed to get artist")?;

                if let Some(id) = existing {
                    return Ok(id);
                }

                let id = diesel::insert_into(artists::table)
                    .values((
                        artists::name.eq(artist),
                        artists::created_at.eq(unix_timestamp()),
                        artists::created_by.eq(uid),
                    ))
                    .returning(artists::id)
                    .get_result(conn)
                    .await
                    .context("Failed to insert artist")?;

                Ok::<_, anyhow::Error>(id)
            }
            .scope_boxed()
        })
        .await?)
}
