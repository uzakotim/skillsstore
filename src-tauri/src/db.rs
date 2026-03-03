use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

pub async fn init_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite:../storage/app.db")
        .await
        .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS books (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            file_path TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        "#
    ).execute(&pool).await.unwrap();

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            book_id TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            content TEXT NOT NULL,
            faiss_id INTEGER NOT NULL
        );
        "#
    ).execute(&pool).await.unwrap();

    pool
}
