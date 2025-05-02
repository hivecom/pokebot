use anyhow::Context;
use axum::extract::Path;
use axum::{Extension, Json};
use diesel::prelude::Queryable;
use diesel::{ExpressionMethods, QueryDsl, Selectable, SelectableHelper};
use diesel_async::RunQueryDsl;
use serde::Serialize;
use ts_rs::TS;
use utoipa::ToSchema;

use crate::db_util::unix_timestamp;
use crate::schema::favourites;
use crate::web_server::api::Error;
use crate::web_server::login::{TsToken, uid_by_token};
use crate::{SqliteConn, SqlitePool};

/// The definition of a favourite
#[derive(Debug, Serialize, TS, ToSchema, Queryable, Selectable)]
#[ts(export, export_to = "../web_server-types/")]
pub struct Favourite {
    #[schema(example = 1)]
    #[ts(type = "number")]
    pub id: i64,

    /// The favourited song
    #[ts(type = "number")]
    pub song_id: i64,

    /// A unix timestamp of when this song was added
    #[schema(example = 1670802822)]
    #[ts(type = "number")]
    pub created_at: i64,
}

pub async fn favourites(conn: &mut SqliteConn, uid: &str) -> Result<Vec<Favourite>, Error> {
    let favs = favourites::table
        .filter(favourites::uid.eq(uid))
        .select(Favourite::as_select())
        .get_results::<Favourite>(conn)
        .await
        .context("Failed to get favourites")?;

    Ok(favs)
}

/// Get favourites
#[utoipa::path(
    get,
    path = "/api/favourite",
    responses(
        (status = 200, description = "Got personal favourites", body = [Favourite])
    )
)]
pub async fn get_all(
    TsToken(token): TsToken,
    Extension(pool): Extension<SqlitePool>,
) -> Result<Json<Vec<Favourite>>, Error> {
    let mut conn = pool.get().await.expect("can connect to sqlite");

    let uid = uid_by_token(&mut conn, &token).await?;

    Ok(Json(favourites(&mut conn, &uid).await?))
}

pub async fn insert(conn: &mut SqliteConn, uid: &str, song_id: i64) -> Result<Favourite, Error> {
    let res = diesel::insert_into(favourites::table)
        .values((
            favourites::song_id.eq(&song_id),
            favourites::uid.eq(uid),
            favourites::created_at.eq(unix_timestamp()),
        ))
        .returning(Favourite::as_select())
        .get_result(conn)
        .await
        .context("Failed to insert Favourite")?;

    Ok(res)
}

/// Post favourite
#[utoipa::path(
    post,
    path = "/api/song/{song_id}/favourite",
    params(
        ("song_id" = i64, Path, description = "The unique ID of the song")
    ),
    responses(
        (status = 201, description = "Set song as favourite", body = Favourite)
    )
)]
pub async fn post(
    Path(song_id): Path<i64>,
    TsToken(token): TsToken,
    Extension(pool): Extension<SqlitePool>,
) -> Result<Json<Favourite>, Error> {
    let mut conn = pool.get().await.expect("can connect to sqlite");

    let uid = uid_by_token(&mut conn, &token).await?;

    Ok(Json(insert(&mut conn, &uid, song_id).await?))
}

pub async fn db_delete(conn: &mut SqliteConn, uid: &str, song_id: i64) -> Result<Favourite, Error> {
    let res = diesel::delete(favourites::table)
        .filter(favourites::song_id.eq(song_id))
        .filter(favourites::uid.eq(uid))
        .returning(Favourite::as_select())
        .get_result(conn)
        .await
        .context("Failed to delete favourite")?;

    Ok(res)
}

/// Delete favourite
#[utoipa::path(
    delete,
    path = "/api/song/{song_id}/favourite",
    params(
        ("song_id" = i64, Path, description = "The unique ID of the favourite")
    ),
    responses(
        (status = 201, description = "Deleted favourite", body = Favourite)
    )
)]
pub async fn delete(
    Path(song_id): Path<i64>,
    TsToken(token): TsToken,
    Extension(pool): Extension<SqlitePool>,
) -> Result<Json<Favourite>, Error> {
    let mut conn = pool.get().await.expect("can connect to sqlite");

    let uid = uid_by_token(&mut conn, &token).await?;

    Ok(Json(db_delete(&mut conn, &uid, song_id).await?))
}
