use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::models::NewBook;

use std::sync::Arc;
use std::time::Duration;

use crate::models::JobStatus;
use crate::AppState;

// The outcome of reading the file: good books + messages about bad rows
pub struct ParseResult {
    pub books: Vec<NewBook>,
    pub errors: Vec<String>,
}

// Reads the CSV and turns each row into a NewBook.
// Plain (non-async) on purpose: we'll run it on a separate thread in the next step.
pub fn parse_books_csv(path: &str) -> Result<ParseResult, csv::Error> {
    let mut reader = csv::Reader::from_path(path)?;
    let mut books = Vec::new();
    let mut errors = Vec::new();

    // deserialize() uses your NewBook struct (via Serde) to read each row
    for (index, record) in reader.deserialize::<NewBook>().enumerate() {
        let line = index + 2; // +1 for the header row, +1 because lines start at 1

        match record {
            Ok(book) => match book.validate() {
                Ok(()) => books.push(book),
                Err(msg) => errors.push(format!("Line {line}: {msg}")),
            },
            Err(err) => errors.push(format!("Line {line}: {err}")),
        }
    }

    Ok(ParseResult { books, errors })
}

// Inserts all books in one transaction: all of them succeed, or none are saved
pub async fn insert_books(pool: &SqlitePool, books: Vec<NewBook>) -> Result<usize, sqlx::Error> {
    let count = books.len();
    let mut tx = pool.begin().await?;

    // `for book in books` CONSUMES the vector: each book is moved out, one at a time
    for book in books {
        sqlx::query(
            "INSERT INTO books (id, title, author, genre, year, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(book.title)
        .bind(book.author)
        .bind(book.genre)
        .bind(book.year)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(count)
}

// Runs in the background. The client is NOT waiting for this.
pub async fn run_import(state: Arc<AppState>, job_id: String) {
    // Artificial 5-second delay so you can watch the status change. Remove it later if you like
    tokio::time::sleep(Duration::from_secs(5)).await;

    // CSV parsing is blocking work, so run it on a dedicated thread
    let parsed = tokio::task::spawn_blocking(|| parse_books_csv("books.csv")).await;

    let (status, inserted, failed, errors) = match parsed {
        Ok(Ok(result)) => {
            let failed = result.errors.len();
            match insert_books(&state.db, result.books).await {
                Ok(inserted) => (JobStatus::Completed, inserted, failed, result.errors),
                Err(e) => (JobStatus::Failed, 0, failed, vec![format!("Database error: {e}")]),
            }
        }
        Ok(Err(e)) => (JobStatus::Failed, 0, 0, vec![format!("Could not read file: {e}")]),
        Err(e) => (JobStatus::Failed, 0, 0, vec![format!("Import task crashed: {e}")]),
    };

    // Lock the shared map, update this job, and release the lock at the end of scope
    let mut jobs = state.jobs.lock().unwrap();
    if let Some(job) = jobs.get_mut(&job_id) {
        job.status = status;
        job.inserted = inserted;
        job.failed = failed;
        job.errors = errors;
    }
}