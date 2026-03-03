mod db;
mod rag;
use rag::embedder::embed;
use rag::faiss::VectorIndex;
use sqlx::{SqlitePool, FromRow};
use serde::Serialize;

#[derive(Serialize, FromRow)]
struct Chunk {
    id: i32,
    book_id: String,
    chunk_index: i32,
    content: String,
}


fn load_or_create_faiss() -> VectorIndex {
    let path = "../storage/faiss.index";
    if std::path::Path::new(path).exists() {
        VectorIndex::load(path)
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

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn upload_pdf(path: String, pool: tauri::State<'_, SqlitePool>)
    -> Result<(), String>
{
    let book_id = uuid::Uuid::new_v4().to_string();
    let title = std::path::Path::new(&path)
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // Save book row
    sqlx::query(
        "INSERT INTO books (id, title, file_path) VALUES (?, ?, ?)"
    )
    .bind(book_id.clone())
    .bind(title)
    .bind(path.clone())
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;


    let text = pdf_extract::extract_text(&path)
        .map_err(|e| e.to_string())?;

    let chunks = chunk_with_overlap(&text, 250, 50);

    let mut index = load_or_create_faiss();
    let mut current_faiss_id = index.ntotal() as i64;

    for (i, chunk) in chunks.iter().enumerate() {
        let embedding = embed(chunk).await?;

        index.add(&embedding);

        sqlx::query(
            "INSERT INTO chunks (book_id, chunk_index, content, faiss_id)
             VALUES (?, ?, ?, ?)"
        )
        .bind(book_id.clone())
        .bind(i as i32)
        .bind(chunk)
        .bind(current_faiss_id)
        .execute(pool.inner())
        .await
        .unwrap();


        current_faiss_id += 1;
    }

    index.save("../storage/faiss.index");

    Ok(())
}
#[tauri::command]
async fn search_context(
    query: String,
    pool: tauri::State<'_, SqlitePool>
) -> Result<Vec<String>, String> {

    let query_embedding = embed(&query).await?;

    let mut index = VectorIndex::load("../storage/faiss.index");
    let ids = index.search(&query_embedding, 5);

    let mut results = Vec::new();

    for id in ids {
        let row: (String,) = sqlx::query_as(
            "SELECT content FROM chunks WHERE faiss_id = ?"
        )
        .bind(id as i64)
        .fetch_one(pool.inner())
        .await
        .unwrap();

        results.push(row.0);
    }


    Ok(results)
}

#[tauri::command]
async fn get_chunks(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<Chunk>, String> {
    sqlx::query_as::<_, Chunk>("SELECT id, book_id, chunk_index, content FROM chunks")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let pool = tauri::async_runtime::block_on(db::init_db());

    tauri::Builder::default()
        .manage(pool)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, upload_pdf, search_context, get_chunks])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
