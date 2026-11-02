use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

// Every way a request can fail in this app
pub enum AppError {
    NotFound,
    BadRequest(String),
    Database(sqlx::Error),
}

// Lets `?` automatically turn a database error into an AppError
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(err)
    }
}

// Tells Axum how to turn an AppError into an HTTP response
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Book not found".to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Database(err) => {
                eprintln!("Database error: {err}"); // log the real error for you
                (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong".to_string())
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}