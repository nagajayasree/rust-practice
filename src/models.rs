use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};

// A fixed set of genres. The JSON "genre" field must be one of these
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Genre {
    Fiction,
    Nonfiction,
    Scifi,
    Fantasy,
    Mystery,
    Biography,
}

// A book as stored in the database and returned by the API
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: String,
    pub genre: Genre,
    pub year: i32,
    pub created_at: DateTime<Utc>,
}

// What the client sends to create a book (no id or created_at; the server sets those)
#[derive(Debug, Deserialize)]
pub struct NewBook {
    pub title: String,
    pub author: String,
    pub genre: Genre,
    pub year: i32,
}

// What the client sends to update a book. Every field is optional
#[derive(Debug, Deserialize)]
pub struct UpdateBook {
    pub title: Option<String>,
    pub author: Option<String>,
    pub genre: Option<Genre>,
    pub year: Option<i32>,
}

// Shared validation rules, used by both create and update
pub fn validate_book(title: &str, author: &str, year: i32) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("Title cannot be empty".to_string());
    }
    if author.trim().is_empty() {
        return Err("Author cannot be empty".to_string());
    }
    let current_year = Utc::now().year();
    if year < 0 || year > current_year {
        return Err(format!("Year must be between 0 and {current_year}"));
    }
    Ok(())
}

// A method on NewBook, like a class method in TS
impl NewBook {
    pub fn validate(&self) -> Result<(), String> {
        validate_book(&self.title, &self.author, self.year)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportJob {
    pub id: String,
    pub status: JobStatus,
    pub inserted: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}