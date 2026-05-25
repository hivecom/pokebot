use std::time::Duration;

use diesel::Queryable;
use std::process::Stdio;
use tokio::process::Command;

use serde::{Deserialize, Serialize};

use tracing::{Span, debug};
use ts_rs::TS;
use utoipa::ToSchema;

use crate::db_util::{schema_opt_duration, serialize_opt_duration};

#[derive(Serialize, Deserialize, Clone, Debug, TS, ToSchema, Queryable)]
#[ts(export, export_to = "../web_server-types/")]
pub struct AudioMetadata {
    #[ts(type = "number")]
    pub id: i64,
    #[serde(rename = "url")]
    pub uri: String,
    pub webpage_url: Option<String>,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub thumbnail: Option<String>,

    #[serde(
        default,
        deserialize_with = "duration_deserialize",
        serialize_with = "serialize_opt_duration"
    )]
    #[schema(example = 170)]
    #[schema(schema_with = schema_opt_duration)]
    #[ts(type = "number | null")]
    pub duration: Option<Duration>,
    #[serde(skip)]
    pub added_by: String,
}

impl AudioMetadata {
    pub fn full_title(&self) -> String {
        format!(
            "{}{}",
            self.title,
            self.album
                .as_ref()
                .map(|a| format!(" - {a}"))
                .unwrap_or_default()
        )
    }
}

fn duration_deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let dur: Option<f64> = Deserialize::deserialize(deserializer)?;

    Ok(dur.map(Duration::from_secs_f64))
}

pub async fn get_audio_download_from_url(
    url: String,
    span: &Span,
) -> Result<AudioMetadata, String> {
    //youtube-dl sometimes just fails, so we give it a second try
    let ytdl_output = match run_youtube_dl(&url, span).await {
        Ok(o) => o,
        Err(e) => {
            if e.contains("Unable to extract video data") {
                run_youtube_dl(&url, span).await?
            } else {
                return Err(e);
            }
        }
    };

    let output = serde_json::from_str(&ytdl_output).map_err(|e| e.to_string())?;

    Ok(output)
}

async fn run_youtube_dl(url: &str, span: &Span) -> Result<String, String> {
    let ytdl_args = ["--no-playlist", "-f", "bestaudio/best", "-j", url];

    let mut command = Command::new("yt-dlp");
    command.args(ytdl_args);
    command.stdin(Stdio::null());

    debug!(parent: span, ?command, "running yt-dlp");
    let ytdl_output = command.output().await.unwrap();

    if !ytdl_output.status.success() {
        let s = String::from_utf8(ytdl_output.stderr).unwrap();
        return Err(s);
    }

    let output_str = String::from_utf8(ytdl_output.stdout).unwrap();

    Ok(output_str)
}
