mod db;
mod rag;
use rag::embedder::embed;
use rag::faiss::VectorIndex;
use sqlx::{SqlitePool, FromRow};
use serde::Serialize;
use tauri::Manager;

#[derive(Serialize, FromRow)]
struct Chunk {
    id: i32,
    book_id: String,
    chunk_index: i32,
    content: String,
}

#[derive(Serialize, FromRow)]
struct Book {
    id: String,
    title: String,
}


struct AppStorage {
    dir: std::path::PathBuf,
}

fn load_or_create_faiss(storage_dir: &std::path::Path) -> VectorIndex {
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

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn upload_pdf(
    path: String, 
    pool: tauri::State<'_, SqlitePool>,
    storage: tauri::State<'_, AppStorage>
) -> Result<(), String> {
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

    let mut index = load_or_create_faiss(&storage.dir);
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
        .map_err(|e| e.to_string())?;


        current_faiss_id += 1;
    }

    index.save(storage.dir.join("faiss.index").to_str().unwrap());

    Ok(())
}
#[tauri::command]
async fn search_context(
    query: String,
    book_id: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
    storage: tauri::State<'_, AppStorage>
) -> Result<Vec<String>, String> {

    let query_embedding = embed(&query).await?;
    let mut index = load_or_create_faiss(&storage.dir);
    let ids = index.search(&query_embedding, 5);

    let mut results = Vec::new();

    for id in ids {
        let query_str = if book_id.is_some() {
            "SELECT content FROM chunks WHERE faiss_id = ? AND book_id = ?"
        } else {
            "SELECT content FROM chunks WHERE faiss_id = ?"
        };

        let mut query = sqlx::query_as::<_, (String,)>(query_str)
            .bind(id as i64);

        if let Some(ref bid) = book_id {
            query = query.bind(bid);
        }

        let row = query.fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

        if let Some(r) = row {
            results.push(r.0);
        }
    }


    Ok(results)
}

#[derive(serde::Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[derive(serde::Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[tauri::command]
async fn generate_response(
    query: String,
    book_id: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
    storage: tauri::State<'_, AppStorage>
) -> Result<String, String> {
    // 1. Get context through search
    let context_results = search_context(query.clone(), book_id, pool, storage).await?;
    let context = context_results.join("\n\n");

    // 2. Build prompt
    let prompt = format!(
        "Use the following pieces of retrieved context to answer the user's question. \
        If you don't know the answer based on the context, just say that you don't know, \
        don't try to make up an answer. Keep the answer concise and relevant.\n\n\
        Context:\n{}\n\nQuestion: {}\n\nAnswer:",
        context, query
    );

    // 3. Call Ollama gemma2:2b
    let client = reqwest::Client::new();
    let res = client
        .post("http://localhost:11434/api/generate")
        .json(&OllamaGenerateRequest {
            model: "gemma2:2b".to_string(),
            prompt,
            stream: false,
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body: OllamaGenerateResponse = res.json().await.map_err(|e| e.to_string())?;

    Ok(body.response)
}


#[tauri::command]
async fn generate_learning_path(
    book_id: String,
    pool: tauri::State<'_, SqlitePool>
) -> Result<String, String> {
    // Check if learning path already exists
    let existing = sqlx::query_as::<_, (String,)>("SELECT content FROM learning_paths WHERE book_id = ?")
        .bind(&book_id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    if let Some(path) = existing {
        return Ok(path.0);
    }

    // Get the first 30 chunks as a proxy for the book's structure and main topics
    let chunks = sqlx::query_as::<_, (String,)>("SELECT content FROM chunks WHERE book_id = ? ORDER BY chunk_index")
        .bind(&book_id)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    let context = chunks.into_iter().map(|c| c.0).collect::<Vec<_>>().join("\n\n");

    let prompt = format!(
        "You are an expert concepts finder.\n\n \
        Based on the context, \
        CONTEXT:\n{}\n\n

        RULES: \
        - Identify the most important valuable concepts, principles and ideas of the book. \n\
        - Be as a teacher for this student. \n\
        - Format the output as a Markdown list. \n\
        CONCEPTS:\n",
        context
    );

    let client = reqwest::Client::new();
    let res = client
        .post("http://localhost:11434/api/generate")
        .json(&OllamaGenerateRequest {
            model: "gemma2:2b".to_string(),
            prompt,
            stream: false,
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body: OllamaGenerateResponse = res.json().await.map_err(|e| e.to_string())?;
    let generated_content = body.response;

    // Store in DB
    sqlx::query("INSERT INTO learning_paths (book_id, content) VALUES (?, ?)")
        .bind(&book_id)
        .bind(&generated_content)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(generated_content)
}

#[tauri::command]
async fn generate_lesson(
    concept: String,
    book_id: String,
    pool: tauri::State<'_, SqlitePool>,
    storage: tauri::State<'_, AppStorage>
) -> Result<String, String> {
    // Check if lesson already exists
    let existing = sqlx::query_as::<_, (String,)>("SELECT content FROM lessons WHERE book_id = ? AND concept = ?")
        .bind(&book_id)
        .bind(&concept)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    if let Some(lesson) = existing {
        return Ok(lesson.0);
    }

    // Search for context about this specific concept
    let context_results = search_context(concept.clone(), Some(book_id.clone()), pool.clone(), storage).await?;
    let context = context_results.join("\n\n");

    let prompt = format!(
        "You are an expert tutor. Using the provided context from the book, \
        explain the concept of '{}' in detail. \
        Provide a structured lesson with clear explanations and examples based on the text.\n\n\
        Context:\n{}\n\nLesson on {}:",
        concept, context, concept
    );

    let client = reqwest::Client::new();
    let res = client
        .post("http://localhost:11434/api/generate")
        .json(&OllamaGenerateRequest {
            model: "gemma2:2b".to_string(),
            prompt,
            stream: false,
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body: OllamaGenerateResponse = res.json().await.map_err(|e| e.to_string())?;
    let generated_content = body.response;

    // Store in DB
    sqlx::query("INSERT INTO lessons (book_id, concept, content) VALUES (?, ?, ?)")
        .bind(&book_id)
        .bind(&concept)
        .bind(&generated_content)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(generated_content)
}

#[tauri::command]
async fn get_stored_learning_path(
    book_id: String,
    pool: tauri::State<'_, SqlitePool>
) -> Result<Option<String>, String> {
    let row = sqlx::query_as::<_, (String,)>("SELECT content FROM learning_paths WHERE book_id = ?")
        .bind(&book_id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(row.map(|r| r.0))
}

#[tauri::command]
async fn get_stored_lessons(
    book_id: String,
    pool: tauri::State<'_, SqlitePool>
) -> Result<Vec<(String, String)>, String> {
    let rows = sqlx::query_as::<_, (String, String)>("SELECT concept, content FROM lessons WHERE book_id = ?")
        .bind(&book_id)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows)
}

#[tauri::command]
async fn get_chunks(
    book_id: Option<String>,
    pool: tauri::State<'_, SqlitePool>
) -> Result<Vec<Chunk>, String> {
    if let Some(bid) = book_id {
        sqlx::query_as::<_, Chunk>("SELECT id, book_id, chunk_index, content FROM chunks WHERE book_id = ?")
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
async fn get_books(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<Book>, String> {
    sqlx::query_as::<_, Book>("SELECT id, title FROM books")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_book(book_id: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    // Delete chunks first due to potential foreign key or just logical grouping
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");
            
            let db_path = app_data_dir.join("app.db");
            let options = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(db_path.clone())
                .create_if_missing(true);
            
            let pool = tauri::async_runtime::block_on(db::init_db(options))
                .unwrap_or_else(|e| panic!("failed to initialize database at {:?}: {}", db_path, e));
            
            app.manage(pool);
            app.manage(AppStorage { dir: app_data_dir });
            
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet, 
            upload_pdf, 
            search_context, 
            get_chunks, 
            get_books, 
            delete_book, 
            generate_response,
            generate_learning_path,
            generate_lesson,
            get_stored_learning_path,
            get_stored_lessons
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
