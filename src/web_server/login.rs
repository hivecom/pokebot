use anyhow::Context;
use axum::{
    Extension, Form, Json,
    body::Body,
    extract::{FromRequestParts, rejection::ExtensionRejection},
    http::{Response, StatusCode},
    response::IntoResponse,
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
    typed_header::TypedHeaderRejection,
};
use diesel::{ExpressionMethods, QueryDsl};
use diesel_async::RunQueryDsl;
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;
use tracing::debug;
use ts_rs::TS;
use utoipa::ToSchema;
use xtra::WeakAddress;

use crate::{SqliteConn, bot::MasterBot, schema::tokens, web_server::api::error::Error};

use super::LoginRequest;

pub async fn uid_by_token(conn: &mut SqliteConn, token: &str) -> anyhow::Result<String> {
    debug!("Trying to get uid by token");
    let uid: String = tokens::table
        .filter(tokens::token.eq(token))
        .select(tokens::uid)
        .get_result(conn)
        .await
        .context("Failed to get token by uid")?;

    Ok(uid)
}

#[derive(Deserialize, Debug, ToSchema, TS)]
#[ts(export, export_to = "../web_server-types/")]
pub struct Login {
    token: String,
}

/// Post login token
#[utoipa::path(
    post,
    path = "/api/login",
    responses(
        (status = 200, description = "Token is valid", body = bool)
    )
)]
pub async fn token(
    Extension(bot): Extension<WeakAddress<MasterBot>>,
    Json(login): Json<Login>,
) -> Result<Json<bool>, Error> {
    if let Ok(uid) = bot.send(LoginRequest(login.token.clone())).await.unwrap() {
        Ok(Json(true))
    } else {
        Ok(Json(false))
    }
}

pub struct TsToken(pub String);

impl<S> FromRequestParts<S> for TsToken
where
    S: Send + Sync,
{
    type Rejection = AuthorizationRejection;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state).await?;

        Ok(TsToken(bearer.token().to_owned()))
    }
}

#[derive(Debug, Error)]
pub enum AuthorizationRejection {
    #[error("{0}")]
    Extension(#[from] ExtensionRejection),
    #[error("{0}")]
    Headers(#[from] TypedHeaderRejection),
    #[error("Invalid session, please login again")]
    InvalidToken,
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Your session has expired, please login again")]
    ExpiredToken,
    #[error("{0}")]
    Generic(#[from] anyhow::Error),
}

impl IntoResponse for AuthorizationRejection {
    fn into_response(self) -> Response<Body> {
        let status = match &self {
            AuthorizationRejection::Extension(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AuthorizationRejection::Generic(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AuthorizationRejection::Headers(_)
            | AuthorizationRejection::InvalidToken
            | AuthorizationRejection::Unauthorized
            | AuthorizationRejection::ExpiredToken => StatusCode::UNAUTHORIZED,
        };

        let body = Json(json!({
            "message": self.to_string(),
        }))
        .into_response();

        (status, body).into_response()
    }
}
