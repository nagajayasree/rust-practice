use chrono::Utc;
use axum::{extract::{Path, State}, http::StatusCode, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{validate_book, Book, NewBook, UpdateBook};
use crate::AppState;

use crate::import::run_import;
use crate::models::{ImportJob, JobStatus};


// POST /books: create a book
pub async fn create_book(
    State(state): State<Arc<AppState>>, // the shared DB pool
    Json(input): Json<NewBook>,         // the JSON body, parsed into NewBook
) -> Result<(StatusCode, Json<Book>), AppError> {
    input.validate().map_err(AppError::BadRequest)?;  

    let book = Book {
        id: Uuid::new_v4().to_string(),
        title: input.title, // moving fields out of `input` into `book`
        author: input.author,
        genre: input.genre,
        year: input.year,
        created_at: Utc::now(),
    };

    sqlx::query(
        "INSERT INTO books (id, title, author, genre, year, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&book.id) // borrow, so `book` is still ours to return
    .bind(&book.title)
    .bind(&book.author)
    .bind(&book.genre)
    .bind(book.year)
    .bind(book.created_at)
    .execute(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(book)))
}

// GET /books: list all books
pub async fn list_books(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Book>>, AppError> {
    let books = sqlx::query_as::<_, Book>(
        "SELECT id, title, author, genre, year, created_at
         FROM books ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(books))
}

// GET /books/{id}: fetch one book
pub async fn get_book(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>, // pulls {id} out of the URL
) -> Result<Json<Book>, AppError> {
    let book = sqlx::query_as::<_, Book>(
        "SELECT id, title, author, genre, year, created_at
         FROM books WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?; // None → 404

    Ok(Json(book))
}

// PUT /books/{id}: update only the fields that were sent
pub async fn update_book(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<UpdateBook>,
) -> Result<Json<Book>, AppError> {
    // 1. Load the current book (or 404)
    let existing = sqlx::query_as::<_, Book>(
        "SELECT id, title, author, genre, year, created_at
         FROM books WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    // 2. Merge: use the new value if sent, otherwise keep the old one
    let updated = Book {
        id: existing.id,
        title: input.title.unwrap_or(existing.title),
        author: input.author.unwrap_or(existing.author),
        genre: input.genre.unwrap_or(existing.genre),
        year: input.year.unwrap_or(existing.year),
        created_at: existing.created_at,
    };

    // 3. Validate the merged result
    validate_book(&updated.title, &updated.author, updated.year)
        .map_err(AppError::BadRequest)?;

    // 4. Save
    sqlx::query("UPDATE books SET title = ?, author = ?, genre = ?, year = ? WHERE id = ?")
        .bind(&updated.title)
        .bind(&updated.author)
        .bind(&updated.genre)
        .bind(updated.year)
        .bind(&updated.id)
        .execute(&state.db)
        .await?;

    Ok(Json(updated))
}

// DELETE /books/{id}: delete one book
pub async fn delete_book(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM books WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        Err(AppError::NotFound) // nothing was deleted, so the id didn't exist
    } else {
        Ok(StatusCode::NO_CONTENT) // 204: deleted, nothing to return
    }
}

// POST /import: start the import in the background and respond immediately
pub async fn import_books(
    State(state): State<Arc<AppState>>,
) -> (StatusCode, Json<ImportJob>) {
    let job = ImportJob {
        id: Uuid::new_v4().to_string(),
        status: JobStatus::Running,
        inserted: 0,
        failed: 0,
        errors: vec![],
    };

    // Record the job as "running"
    state.jobs.lock().unwrap().insert(job.id.clone(), job.clone());

    // Give the background task its OWN handles to the state and job id
    let task_state = Arc::clone(&state); // +1 owner of the same AppState, not a copy
    let task_job_id = job.id.clone();

    // `async move` moves task_state and task_job_id INTO the task
    tokio::spawn(async move {
        run_import(task_state, task_job_id).await;
    });

    (StatusCode::ACCEPTED, Json(job)) // returns before the import finishes
}

// GET /import/{id}: check on a job
pub async fn get_import_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ImportJob>, AppError> {
    let job = state.jobs.lock().unwrap().get(&id).cloned();
    job.map(Json).ok_or(AppError::NotFound)
}