use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ApiError>;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Dashboard not found")]
    DashboardNotFound,

    #[error("Snapshot not found for date: {0}")]
    SnapshotNotFound(String),

    #[error("Invalid date format: {0}")]
    InvalidDateFormat(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::DashboardNotFound => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::SnapshotNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::InvalidDateFormat(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::IoError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ApiError::JsonError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
