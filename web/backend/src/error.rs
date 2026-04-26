use axum::{http::StatusCode, response::IntoResponse};
use tracing::{error, warn};

/// Represents an error that can be returned by the API handlers.
/// Implements `IntoResponse` to automatically convert into an HTTP response.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    pub message: String,
}

impl ApiError {
    /// Creates a new `ApiError` with the given HTTP status code and message.
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    /// Convenience method to create an internal server error (500) from any type that implements `Display`.
    pub fn internal(err: impl std::fmt::Display) -> Self {
        let msg = err.to_string();
        error!(error = %msg, "Internal server error");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, msg)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        if self.status.is_client_error() {
            warn!(status = %self.status, message = %self.message, "Client error");
        }
        (self.status, self.message).into_response()
    }
}
