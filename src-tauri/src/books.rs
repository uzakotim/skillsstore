use crate::ollama::{embed_with_model, ModelConfigState};
use crate::rag::faiss::VectorIndex;
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

#[derive(Serialize, FromRow)]
pub struct Chunk {
    pub id: i32,
    pub book_id: String,
    pub chunk_index: i32,
    pub content: String,
}

#[derive(Serialize, FromRow)]
pub struct Book {
    pub id: String,
    pub title: String,
}

pub struct AppStorage {
    pub dir: std::path::PathBuf,
}

pub fn load_or_create_faiss(storage_dir: &std::path::Path) -> VectorIndex {
    let path = storage_dir.join("faiss.index");
    if path.exists() {
        VectorIndex::load(path.to_str().unwrap())
    } else {
        VectorIndex::new(768) // nomic-embed-text dimension
    }
}

pub fn chunk_with_overlap(text: &str, size: usize, overlap: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut chunks = Vec::new();
    let mut i = 0;

    while i < words.len() {
        let end = usize::min(i + size, words.len());
        chunks.push(words[i..end].join(" "));
        i += size - overlap;
    }

    chunks
}

#[tauri::command]
pub async fn upload_pdf(
    path: String,
    pool: tauri::State<'_, SqlitePool>,
    storage: tauri::State<'_, AppStorage>,
    model_config: tauri::State<'_, ModelConfigState>,
) -> Result<(), String> {
    let book_id = uuid::Uuid::new_v4().to_string();
    let title = std::path::Path::new(&path)
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    sqlx::query("INSERT INTO books (id, title, file_path) VALUES (?, ?, ?)")
        .bind(book_id.clone())
        .bind(title)
        .bind(path.clone())
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    let text = pdf_extract::extract_text(&path).map_err(|e| e.to_string())?;

    if text.trim().is_empty() {
        return Err("No text extracted. PDF might be scanned/image-only.".into());
    }

    let chunks = chunk_with_overlap(&text, 250, 50);

    let embed_model = {
        let cfg = model_config.config.lock().unwrap();
        cfg.embed_model.clone()
    };

    let mut index = load_or_create_faiss(&storage.dir);
    let mut current_faiss_id = index.ntotal() as i64;

    for (i, chunk) in chunks.iter().enumerate() {
        let embedding = embed_with_model(chunk, &embed_model).await?;

        index.add(&embedding);

        sqlx::query(
            "INSERT INTO chunks (book_id, chunk_index, content, faiss_id)
             VALUES (?, ?, ?, ?)",
        )
        .bind(book_id.clone())
        .bind(i as i32)
        .bind(chunk)
        .bind(current_faiss_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

        current_faiss_id += 1;
    }

    index.save(storage.dir.join("faiss.index").to_str().unwrap());
    Ok(())
}

#[tauri::command]
pub async fn search_context(
    query: String,
    book_id: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
    storage: tauri::State<'_, AppStorage>,
    model_config: tauri::State<'_, ModelConfigState>,
) -> Result<Vec<String>, String> {
    let embed_model = {
        let cfg = model_config.config.lock().unwrap();
        cfg.embed_model.clone()
    };

    let query_embedding = embed_with_model(&query, &embed_model).await?;
    let mut index = load_or_create_faiss(&storage.dir);
    let ids = index.search(&query_embedding, 5);

    let mut results = Vec::new();

    for id in ids {
        let query_str = if book_id.is_some() {
            "SELECT content FROM chunks WHERE faiss_id = ? AND book_id = ?"
        } else {
            "SELECT content FROM chunks WHERE faiss_id = ?"
        };

        let mut q = sqlx::query_as::<_, (String,)>(query_str).bind(id as i64);
        if let Some(ref bid) = book_id {
            q = q.bind(bid);
        }

        let row = q
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
        if let Some(r) = row {
            results.push(r.0);
        }
    }

    Ok(results)
}

#[tauri::command]
pub async fn get_chunks(
    book_id: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<Chunk>, String> {
    if let Some(bid) = book_id {
        sqlx::query_as::<_, Chunk>(
            "SELECT id, book_id, chunk_index, content FROM chunks WHERE book_id = ?",
        )
        .bind(bid)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())
    } else {
        sqlx::query_as::<_, Chunk>("SELECT id, book_id, chunk_index, content FROM chunks")
            .fetch_all(pool.inner())
            .await
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn get_books(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<Book>, String> {
    sqlx::query_as::<_, Book>("SELECT id, title FROM books")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_book(book_id: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("DELETE FROM chunks WHERE book_id = ?")
        .bind(&book_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM books WHERE id = ?")
        .bind(&book_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
