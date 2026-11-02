use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    // Creates bookshelf.db in the project folder if it doesn't exist
    let options = SqliteConnectOptions::from_str("sqlite:bookshelf.db")?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS books (
            id          TEXT PRIMARY KEY,
            title       TEXT NOT NULL,
            author      TEXT NOT NULL,
            genre       TEXT NOT NULL,
            year        INTEGER NOT NULL,
            created_at  TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool) // borrow the pool; we still need it afterwards
    .await?;

    Ok(pool)
}