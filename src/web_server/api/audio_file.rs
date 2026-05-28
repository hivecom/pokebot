use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, anyhow};
use axum::body::Bytes;
use axum::extract::Multipart;
use axum::http::StatusCode;
use axum::{Extension, Json};
use diesel::prelude::Queryable;
use diesel::{ExpressionMethods, QueryDsl, Selectable, SelectableHelper};
use diesel_async::scoped_futures::ScopedFutureExt;
use diesel_async::{AsyncConnection, RunQueryDsl};
use lofty::file::{AudioFile as LoftyAudioFile, TaggedFileExt};
use lofty::picture::PictureType;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use serde::Serialize;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use ts_rs::TS;
use utoipa::ToSchema;

use crate::db_util::{DbDuration, schema_duration, serialize_duration, unix_timestamp};
use crate::schema::audio_files;
use crate::web_server::ConfigVars;
use crate::web_server::api::Error;
use crate::web_server::login::{TsToken, uid_by_token};
use crate::{SqliteConn, SqlitePool};

/// The definition of a song.
#[derive(Debug, Serialize, TS, Queryable, ToSchema, Selectable)]
#[ts(export, export_to = "../web_server-types/")]
pub struct AudioFile {
    #[schema(example = 1)]
    #[ts(type = "number")]
    pub id: i64,

    pub file_name: String,
    pub file_path: String,

    #[schema(example = 170)]
    #[serde(serialize_with = "serialize_duration")]
    #[diesel(deserialize_as = DbDuration)]
    #[schema(schema_with = schema_duration)]
    #[ts(type = "number")]
    pub duration: Duration,

    /// A unix timestamp of when this song was added
    #[schema(example = 1670802822)]
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Debug, Default, Serialize, TS, ToSchema)]
#[ts(export, export_to = "../web_server-types/")]
pub struct SongMetadata {
    #[ts(type = "number | null")]
    pub track: Option<i64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub cover_path: Option<String>,

    /// The duration of the song in milliseconds
    #[schema(example = 170)]
    #[serde(serialize_with = "serialize_duration")]
    #[schema(schema_with = schema_duration)]
    #[ts(type = "number")]
    pub duration: Duration,
}

pub async fn files(conn: &mut SqliteConn) -> Result<Vec<AudioFile>, Error> {
    let files = audio_files::table
        .select(AudioFile::as_select())
        .get_results(conn)
        .await
        .context("Failed to get audio files")?;

    Ok(files)
}

/// Get songs
#[utoipa::path(
    get,
    path = "/api/audio",
    responses(
        (status = 200, description = "Got all files", body = [AudioFile])
    )
)]
pub async fn get_all(
    Extension(pool): Extension<SqlitePool>,
) -> Result<Json<Vec<AudioFile>>, Error> {
    let mut conn = pool.get().await.expect("can connect to sqlite");

    Ok(Json(files(&mut conn).await?))
}

#[derive(Debug, Default, Serialize, TS, ToSchema)]
#[ts(export, export_to = "../web_server-types/")]
pub struct UploadResponse {
    #[schema(example = 1)]
    #[ts(type = "number")]
    pub file_id: i64,
    pub metadata: SongMetadata,
}

#[derive(ToSchema)]
pub struct FileForm {
    #[schema(content_media_type = "application/octet-stream")]
    pub file: Vec<u8>,
}

/// Upload audio file
#[utoipa::path(
    post,
    path = "/api/audio",
    request_body(content = FileForm, description = "Multipart file", content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "File was successfully uploaded", body = UploadResponse)
    )
)]
pub async fn upload(
    TsToken(token): TsToken,
    Extension(pool): Extension<SqlitePool>,
    Extension(vars): Extension<ConfigVars>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<UploadResponse>), Error> {
    let mut conn = pool.get().await.expect("can connect to sqlite");

    let mut file_name = None;
    let mut data = None;
    while let Some(field) = multipart.next_field().await.unwrap() {
        if let Some("file") = field.name() {
            file_name = Some(
                field
                    .file_name()
                    .context("Failed to get file name")?
                    .to_owned(),
            );
            data = Some(field.bytes().await.context("Failed to get song data")?);
            break;
        }
    }

    let uid = uid_by_token(&mut conn, &token).await?;
    let data = data.ok_or(Error::RequiredField("file"))?;
    let file_name = file_name.ok_or(Error::RequiredField("file"))?;
    let metadata = metadata(&data).await?;
    let file_path = generate_file_path(file_name.split('.').next_back().unwrap_or("mp3"));

    let file_id = conn
        .transaction(|conn| {
            async move {
                let file_id: i64 = diesel::insert_into(audio_files::table)
                    .values((
                        audio_files::file_name.eq(&file_name),
                        audio_files::file_path
                            .eq(&file_path.to_str().expect("FIXME: paths can be non UTF-8")),
                        audio_files::duration.eq(DbDuration::from(metadata.duration)),
                        audio_files::created_at.eq(unix_timestamp()),
                        audio_files::created_by.eq(uid),
                    ))
                    .returning(audio_files::id)
                    .get_result(conn)
                    .await
                    .context("Failed to insert audio file")?;

                let mut absolute_path = vars.music_root;
                absolute_path.push(file_path);
                fs::create_dir_all(&absolute_path.parent().unwrap())
                    .await
                    .unwrap();
                fs::write(&absolute_path, &data).await.unwrap();

                Ok::<_, anyhow::Error>(file_id)
            }
            .scope_boxed()
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(UploadResponse { file_id, metadata }),
    ))
}

pub async fn metadata(data: &Bytes) -> Result<SongMetadata, Error> {
    let probe = Probe::new(Cursor::new(data))
        .guess_file_type()
        .context("Failed to guess file type")?;
    let file = probe.read().context("Failed to read song for tags")?;
    if let Some(tag) = file.primary_tag() {
        let mut cover = None;
        for picture in tag.pictures() {
            if picture.pic_type() == PictureType::CoverFront {
                // The image type might be wrong but it does not seem like the big browsers
                // care so finding the correct type does not seem like it is worth the effort.
                cover = Some(picture.data());
            }
        }

        let track = tag.track().map(|i| i as i64);
        let title = tag.title().map(|t| t.to_string());
        let artist = tag.artist().map(|a| a.to_string());
        let album = tag.album().map(|a| a.to_string());

        let cover_path = match cover {
            Some(cover) => {
                let cover_path = generate_cover_path();
                let mut file = File::create_new(&cover_path)
                    .await
                    .context("Failed to create cover image")?;
                file.write(cover)
                    .await
                    .context("Failed to create cover image")?;

                // FIXME: not great to use lossy on a path but it should be fine since it's just a uuid
                Some(cover_path.to_string_lossy().to_string())
            }
            None => None,
        };

        Ok(SongMetadata {
            track,
            title,
            artist,
            album,
            cover_path,
            duration: file.properties().duration(),
        })
    } else {
        Ok(SongMetadata {
            track: None,
            title: None,
            artist: None,
            album: None,
            cover_path: None,
            duration: file.properties().duration(),
        })
    }
}

fn generate_file_path(extension: &str) -> PathBuf {
    let mut file_path = PathBuf::new();
    file_path.push(format!("{}.{}", uuid::Uuid::new_v4(), extension));

    file_path
}

fn generate_cover_path() -> PathBuf {
    let mut cover_path = PathBuf::from("covers");
    cover_path.push(format!("{}.{}", uuid::Uuid::new_v4(), "jpg"));

    cover_path
}
