# Bookshelf API

A REST API for managing books, built in **Rust** with **Axum**, **Tokio**, and **SQLx** (SQLite).

It supports full CRUD on books, input validation, bulk import from a CSV file, and background processing, all built on Rust's ownership model for safe concurrency.

---

## Features

- **8 REST endpoints** for books and imports
- **Full CRUD** against a SQLite database with parameterized queries
- **Custom data models** with type-level validation (e.g. a `Genre` enum)
- **Partial updates**: send only the fields you want to change
- **CSV import** that skips and reports bad rows with line numbers
- **Background jobs**: imports run in a separate task, with status tracking
- **Consistent JSON errors** with correct HTTP status codes

---

## Tech Stack

| Crate | Purpose |
|---|---|
| [axum](https://crates.io/crates/axum) | Web framework: routing, extractors, responses |
| [tokio](https://crates.io/crates/tokio) | Async runtime, background tasks, blocking-thread pool |
| [sqlx](https://crates.io/crates/sqlx) | Async database access (SQLite), row → struct mapping |
| [serde](https://crates.io/crates/serde) / serde_json | JSON and CSV (de)serialization |
| [csv](https://crates.io/crates/csv) | CSV file parsing |
| [uuid](https://crates.io/crates/uuid) | Unique IDs for books and jobs |
| [chrono](https://crates.io/crates/chrono) | Timestamps |

---

## Getting Started

### Prerequisites
- Rust (install via [rustup.rs](https://rustup.rs))

### Run
```bash
git clone <your-repo-url>
cd bookshelf-api
cargo run
```

The server starts on **http://127.0.0.1:3000**. The SQLite database (`bookshelf.db`) and the `books` table are created automatically on first run.

---

## API Endpoints

| Method | Path | Description | Success |
|---|---|---|---|
| GET | `/health` | Health check | 200 |
| GET | `/books` | List all books (newest first) | 200 |
| POST | `/books` | Create a book | 201 |
| GET | `/books/{id}` | Get one book | 200 |
| PUT | `/books/{id}` | Update a book (partial) | 200 |
| DELETE | `/books/{id}` | Delete a book | 204 |
| POST | `/import` | Start a background CSV import | 202 |
| GET | `/import/{id}` | Check import job status | 200 |

### Book model

```json
{
  "id": "9f8ac576-122e-4642-a240-a983b0b39e1f",
  "title": "Dune",
  "author": "Frank Herbert",
  "genre": "scifi",
  "year": 1965,
  "created_at": "2026-10-18T22:21:33.891990Z"
}
```

Allowed genres: `fiction`, `nonfiction`, `scifi`, `fantasy`, `mystery`, `biography`

### Examples

**Create a book**
```bash
curl -X POST http://127.0.0.1:3000/books \
  -H "Content-Type: application/json" \
  -d '{"title":"Dune","author":"Frank Herbert","genre":"scifi","year":1965}'
```

**List books**
```bash
curl http://127.0.0.1:3000/books
```

**Get one book**
```bash
curl http://127.0.0.1:3000/books/<id>
```

**Update only the year**
```bash
curl -X PUT http://127.0.0.1:3000/books/<id> \
  -H "Content-Type: application/json" \
  -d '{"year":1966}'
```

**Delete a book**
```bash
curl -X DELETE http://127.0.0.1:3000/books/<id>
```

**Start an import, then check its status**
```bash
curl -X POST http://127.0.0.1:3000/import
# → 202 Accepted, {"id":"<job_id>","status":"running",...}

curl http://127.0.0.1:3000/import/<job_id>
# → {"status":"completed","inserted":10,"failed":3,"errors":[...]}
```

### Errors

All errors return JSON in the form `{"error": "message"}`:

| Status | When |
|---|---|
| 400 Bad Request | Validation failed (empty title or author, invalid year) |
| 404 Not Found | Book or import job doesn't exist |
| 422 Unprocessable Entity | Malformed JSON or invalid genre |
| 500 Internal Server Error | Database error (details are logged, not exposed) |

---

## Project Structure

```
bookshelf-api/
├── Cargo.toml       # Dependencies
├── books.csv        # Sample data for the import endpoint
└── src/
    ├── main.rs      # Server setup, shared state, routes
    ├── models.rs    # Data models and validation
    ├── db.rs        # Database connection pool and table setup
    ├── routes.rs    # Endpoint handlers
    ├── errors.rs    # AppError → HTTP responses
    └── import.rs    # CSV parsing, bulk insert, background import job
```

**Request flow:** request → `main.rs` router → handler in `routes.rs` → models from `models.rs` + DB pool from `db.rs` → JSON response (or `AppError` from `errors.rs`).

---

## How It Works

### Database and CRUD
- On startup, `db.rs` opens a **connection pool** (up to 5 connections) and creates the `books` table if needed.
- The pool lives in a shared `AppState`, wrapped in `Arc`, and every handler can access it.
- All queries use `.bind(...)` (parameterized), which prevents SQL injection.
- `query_as::<_, Book>` maps rows directly into the `Book` struct via `#[derive(FromRow)]`.
- Missing records return 404 via `fetch_optional` (reads) and `rows_affected()` (deletes).

### Custom Data Models
| Type | Purpose |
|---|---|
| `Book` | A stored book, as returned by the API |
| `NewBook` | Create input: no `id` or `created_at`, so clients can't set server-controlled fields |
| `UpdateBook` | Update input: every field is `Option`, enabling partial updates |
| `Genre` | Enum restricting genre to valid values at the type level |
| `ImportJob` / `JobStatus` | Background import tracking |
| `AppError` | Every failure case, converted to an HTTP response via `IntoResponse` |

Validation lives on the model (`NewBook::validate`) and is shared by the create, update, and CSV import paths.

### File Reading (CSV Import)
1. `parse_books_csv` reads `books.csv` and deserializes each row directly into `NewBook`, the same model the API uses.
2. Rows that fail to parse (e.g. an unknown genre) or fail validation (e.g. an empty title) are **skipped and reported with line numbers**.
3. Valid rows are inserted in a single **transaction**, so the import is all-or-nothing.

### Async and Multi-threading
`POST /import` demonstrates explicit concurrency:
1. The handler records a job as `running` in a `Mutex<HashMap<String, ImportJob>>`.
2. It starts the import with **`tokio::spawn`** and returns **202 Accepted** immediately.
3. In the background, CSV parsing runs in **`spawn_blocking`**, a dedicated thread for blocking work, so Tokio's async workers stay free for requests.
4. When the import finishes, the task locks the job map and sets the status to `completed` or `failed`.
5. Other endpoints keep responding throughout, and `GET /import/{id}` shows progress.

All handlers are `async` and run on Tokio's **multi-threaded runtime**.

---

## Ownership in This Project

Rust gives every value a single owner and frees it when that owner goes out of scope, without a garbage collector. Values can be **moved** or **borrowed** (many `&T` or one `&mut T`), and the compiler enforces these rules at compile time, preventing data races and use-after-free bugs.

| Concept | Where | What happens |
|---|---|---|
| **Move** | `AppState { db: pool }` in `main.rs` | The pool now belongs to `AppState`; using `pool` afterward won't compile |
| **Move** | `title: input.title` in `create_book` | The string moves from the request into the new `Book` with no copy |
| **Borrow** | `.bind(&book.id)` | The query reads the value, and `book` is still returned afterward |
| **Borrow** | `validate_book(&title, &author, year)` | Validation inspects the data without taking ownership |
| **Consume** | `for book in books` in `insert_books` | The loop takes ownership of each book in turn |
| **Shared ownership** | `Arc<AppState>`, `Arc::clone(&state)` | Many handlers and tasks share one state; it's freed when the last owner is dropped |
| **Move into a task** | `tokio::spawn(async move { ... })` | The background task owns its data, because it can outlive the handler that started it |
| **Automatic cleanup** | `state.jobs.lock()` | The `Mutex` unlocks when the guard goes out of scope, with no manual unlock |

---

## Possible Improvements
- Pagination and filtering on `GET /books` (e.g. `?genre=scifi`)
- Reviews endpoints (`/books/{id}/reviews`)
- Integration tests
- Uploading a CSV file instead of reading a fixed path
- Persisting import jobs in the database instead of memory
