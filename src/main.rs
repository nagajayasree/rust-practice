mod db;
mod errors;
mod import;
mod models;
mod routes;

use axum::{routing::{get, post}, Json, Router};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Shared state every handler can access
pub struct AppState {
    pub db: SqlitePool,
    pub jobs: Mutex<HashMap<String, models::ImportJob>>, // job_id → job
}

#[tokio::main]
async fn main() {
    let pool = db::init_db().await.expect("Failed to set up database");
    println!("Database ready");

    // `pool` is MOVED into AppState, then wrapped in Arc so handlers can share it
    let state = Arc::new(AppState {
        db: pool,
        jobs: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/books", get(routes::list_books).post(routes::create_book))
        .route(
            "/books/{id}",
            get(routes::get_book)
                .put(routes::update_book)
                .delete(routes::delete_book),
        )
        .route("/import", post(routes::import_books))
        .route("/import/{id}", get(routes::get_import_status))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("Server running on http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}