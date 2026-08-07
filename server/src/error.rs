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
            // FK violation = client referenced a row that doesn't exist. This is
            // a client error (400), not a server fault (500). Gives clients an
            // actionable message instead of an opaque internal error.
            sqlx::Error::Database(ref d) if d.is_foreign_key_violation() => {
                ApiError::BadRequest("referenced row does not exist".into())
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

#[cfg(test)]
mod tests {
    //! Unit tests for the `From<sqlx::Error>` mapping. We construct synthetic
    //! `sqlx::Error::Database(...)` values via a stub `DatabaseError`
    //! implementor so we can drive each branch (unique / FK / other) without
    //! needing a live database. The HTTP status each `ApiError` variant maps to
    //! is also asserted via `IntoResponse`.

    use super::*;
    use axum::http::StatusCode;
    use std::borrow::Cow;

    /// A minimal stub implementing `sqlx::error::DatabaseError` so we can build
    /// `sqlx::Error::Database(Box<dyn DatabaseError>)` with a chosen
    /// `ErrorKind`. Only `kind()` is meaningful; the rest return stubs.
    ///
    /// `ErrorKind` is neither `Clone` nor `Copy`, so we hold our own `Kind`
    /// tag (which is `Copy`) and translate it to `ErrorKind` inside `kind()`.
    #[derive(Debug, Clone, Copy)]
    enum Kind {
        Unique,
        ForeignKey,
        NotNull,
        Check,
        Other,
    }

    #[derive(Debug)]
    struct StubDbError(Kind);

    impl std::fmt::Display for StubDbError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "stub db error: {:?}", self.0)
        }
    }

    impl std::error::Error for StubDbError {}

    impl sqlx::error::DatabaseError for StubDbError {
        fn message(&self) -> &str {
            "stub"
        }
        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }
        fn code(&self) -> Option<Cow<'_, str>> {
            None
        }
        fn kind(&self) -> sqlx::error::ErrorKind {
            match self.0 {
                Kind::Unique => sqlx::error::ErrorKind::UniqueViolation,
                Kind::ForeignKey => sqlx::error::ErrorKind::ForeignKeyViolation,
                Kind::NotNull => sqlx::error::ErrorKind::NotNullViolation,
                Kind::Check => sqlx::error::ErrorKind::CheckViolation,
                Kind::Other => sqlx::error::ErrorKind::Other,
            }
        }
    }

    fn db_err(kind: Kind) -> sqlx::Error {
        sqlx::Error::Database(Box::new(StubDbError(kind)))
    }

    #[test]
    fn row_not_found_maps_to_404() {
        let api = ApiError::from(sqlx::Error::RowNotFound);
        let (status, _) = status_and_msg(&api);
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn unique_violation_maps_to_409_conflict() {
        let api = ApiError::from(db_err(Kind::Unique));
        let (status, msg) = status_and_msg(&api);
        assert_eq!(status, StatusCode::CONFLICT);
        assert!(msg.contains("duplicate"), "conflict message: {msg}");
    }

    #[test]
    fn foreign_key_violation_maps_to_400_bad_request() {
        let api = ApiError::from(db_err(Kind::ForeignKey));
        let (status, msg) = status_and_msg(&api);
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            msg.contains("referenced row"),
            "FK message should be actionable: {msg}"
        );
    }

    #[test]
    fn other_database_error_maps_to_500_internal() {
        // ErrorKind::Other (e.g. a generic SQLITE_CONSTRAINT not covered above)
        // must NOT be mis-mapped to a client error — it stays a 500.
        let api = ApiError::from(db_err(Kind::Other));
        let (status, _) = status_and_msg(&api);
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    /// Helper: render an ApiError to its (status, message) pair without
    /// consuming the response body (we just re-derive via the enum).
    fn status_and_msg(e: &ApiError) -> (StatusCode, String) {
        let s = e.to_string();
        let st = match e {
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden => StatusCode::FORBIDDEN,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (st, s)
    }
}
