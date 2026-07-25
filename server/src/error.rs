//! Error → HTTP response mapping.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("not found")]
    NotFound,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => ApiError::NotFound,
            sqlx::Error::Database(ref d) if d.is_unique_violation() => {
                ApiError::Conflict("duplicate value".into())
            }
            _ => ApiError::Internal(e.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, self.to_string()),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::Conflict(_) => (StatusCode::CONFLICT, self.to_string()),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
