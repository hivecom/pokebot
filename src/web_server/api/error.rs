use axum::{
    Json,
    body::Body,
    extract::rejection::JsonRejection,
    http::{Response, StatusCode},
    response::IntoResponse,
};
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Not Found")]
    NotFound,

    #[error("Field '{0}' is missing")]
    RequiredField(&'static str),

    #[error("{0}")]
    Internal(#[from] anyhow::Error),

    #[error("{0}")]
    JsonRejection(#[from] JsonRejection),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response<Body> {
        let status = match self {
            Error::NotFound => StatusCode::NOT_FOUND,
            Error::RequiredField(_) | Error::JsonRejection(_) => StatusCode::BAD_REQUEST,
            Error::Internal(ref e) => {
                let err = e
                    .chain()
                    .skip(1)
                    .fold(e.to_string(), |acc, cause| format!("{}: {}", acc, cause));
                error!("API encountered error: {}", err);

                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        let message = if let Error::JsonRejection(rej) = self {
            use std::error::Error;
            match rej {
                JsonRejection::JsonDataError(e) => e.source().unwrap().to_string(),
                JsonRejection::JsonSyntaxError(e) => e.source().unwrap().to_string(),
                _ => rej.to_string(),
            }
        } else {
            self.to_string()
        };

        let body = Json(message);

        (status, body).into_response()
    }
}
