use std::{fs::File, io::Read, path::Path};

use anyhow::Error;
use axum::body::Bytes;
use diesel::{ExpressionMethods, OptionalExtension};
use diesel_async::RunQueryDsl;
use tracing::{debug, error, info};

use crate::{
    SqlitePool,
    db_util::{DbDuration, unix_timestamp},
    schema::audio_files,
    web_server::api::{
        audio_file::metadata,
        song::{PutSongMetadata, update_song_metadata},
    },
};

pub async fn scan_music(music_root: &Path, pool: &SqlitePool) {
    let mut conn = pool.get().await.expect("can connect to sqlite");

    for entry in walkdir::WalkDir::new(music_root) {
        if let Ok(entry) = entry
            && entry.file_type().is_file()
        {
            let file_path = entry.path().strip_prefix(music_root).unwrap();
            let file_name = entry.file_name();

            dbg!(&file_path);
            let mut file = File::open(entry.path()).unwrap();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            let data = Bytes::from_owner(data);
            match metadata(&data).await {
                Ok(metadata) => {
                    debug!(path = ?file_path, "Found metadata");
                    let file_id: Option<i64> = diesel::insert_into(audio_files::table)
                        .values((
                            audio_files::file_name.eq(&file_name.to_str().unwrap()),
                            audio_files::file_path
                                .eq(&file_path.to_str().expect("FIXME: paths can be non UTF-8")),
                            audio_files::duration.eq(DbDuration::from(metadata.duration)),
                            audio_files::created_at.eq(unix_timestamp()),
                            audio_files::created_by.eq("cli-scan"),
                        ))
                        .on_conflict(audio_files::file_path)
                        .do_nothing()
                        .returning(audio_files::id)
                        .get_result(&mut conn)
                        .await
                        .optional()
                        .unwrap();

                    if let Some(title) = metadata.title
                        && let Some(artist) = metadata.artist
                        && let Some(file_id) = file_id
                    {
                        debug!("Updating metadata");
                        let song = update_song_metadata(
                            &mut conn,
                            "cli-scan",
                            file_id,
                            PutSongMetadata {
                                track: metadata.track,
                                title,
                                artist,
                                album: metadata.album,
                                cover_path: metadata.cover_path,
                                disc_number: metadata.disc_number,
                            },
                        )
                        .await
                        .unwrap();
                        info!("Added {} - {}", song.title, song.artist);
                    }
                }
                Err(e) => {
                    let e = Error::new(e);
                    let err = e
                        .chain()
                        .skip(1)
                        .fold(e.to_string(), |acc, cause| format!("{}: {}", acc, cause));
                    error!(path=?file_path, error = %err, "Failed to find metadata");
                }
            }
        }
    }
}
