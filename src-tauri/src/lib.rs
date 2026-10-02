mod db;
mod rag;
// rag::embedder::embed is superseded by embed_with_model below
use rag::faiss::VectorIndex;
use serde::{Deserialize, Serialize};
use sqlx::{Acquire, FromRow, SqlitePool};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::Emitter;
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

// ─── Ollama API Types ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OllamaListResponse {
    models: Vec<OllamaModelInfo>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OllamaModelInfo {
    name: String,
    size: Option<u64>,
    digest: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OllamaTag {
    name: String,
    description: Option<String>,
    #[serde(rename = "pulls")]
    pulls: Option<u64>,
}

#[derive(Serialize)]
struct OllamaPullRequest {
    name: String,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ModelConfig {
    llm_model: String,
    embed_model: String,
}

#[derive(Deserialize)]
struct OllamaPullProgress {
    status: String,
    completed: Option<u64>,
    total: Option<u64>,
}

#[derive(Serialize, Clone)]
pub struct PullProgress {
    status: String,
    completed: Option<u64>,
    total: Option<u64>,
    percent: Option<f64>,
}

#[derive(Serialize, Clone)]
pub struct AnalysisProgress {
    pub status: String,
    pub processed: Option<u64>,
    pub total: Option<u64>,
    pub percent: Option<f64>,
}

// ─── Model Config Storage (in-memory + persisted to a simple JSON file) ────────

pub struct ModelConfigState {
    pub config: Mutex<ModelConfig>,
    pub config_path: std::path::PathBuf,
}

impl ModelConfigState {
    pub fn load(storage_dir: &std::path::Path) -> Self {
        let path = storage_dir.join("model_config.json");

        let config = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                    eprintln!(
                        "Failed to parse model_config.json at {:?}: {}, falling back to defaults",
                        path, e
                    );
                    ModelConfig {
                        llm_model: "gemma2:2b".to_string(),
                        embed_model: "nomic-embed-text".to_string(),
                    }
                }),
                Err(e) => {
                    eprintln!("Failed to read model_config.json at {:?}: {}", path, e);
                    ModelConfig {
                        llm_model: "gemma2:2b".to_string(),
                        embed_model: "nomic-embed-text".to_string(),
                    }
                }
            }
        } else {
            let default_cfg = ModelConfig {
                llm_model: "gemma2:2b".to_string(),
                embed_model: "nomic-embed-text".to_string(),
            };
            // Persist the default configuration immediately so the file exists
            if let Ok(json) = serde_json::to_string_pretty(&default_cfg) {
                let _ = std::fs::write(&path, json);
            }
            default_cfg
        };

        Self {
            config: Mutex::new(config),
            config_path: path,
        }
    }

    pub fn save(&self) {
        if let Ok(config) = self.config.lock() {
            if let Ok(json) = serde_json::to_string_pretty(&*config) {
                if let Err(e) = std::fs::write(&self.config_path, json) {
                    eprintln!(
                        "Failed to write model_config.json to {:?}: {}",
                        self.config_path, e
                    );
                }
            }
        }
    }
}
struct OllamaProcess {
    child: Arc<Mutex<Option<Child>>>,
}

impl OllamaProcess {
    fn new() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
        }
    }
}
// ─── Ollama Tauri Commands ────────────────────────────────────────────────────
#[tauri::command]
fn start_ollama(
    app: tauri::AppHandle,
    state: tauri::State<'_, OllamaProcess>,
) -> Result<(), String> {
    let mut guard = state
        .child
        .lock()
        .map_err(|_| "Failed to lock Ollama process state".to_string())?;

    // Already owned by this app.
    if guard.is_some() {
        return Ok(());
    }

    // Kill anything that was started outside this app.
    kill_existing_ollama();
    let ollama_path = find_ollama()?;
    let mut cmd = Command::new(ollama_path);
    cmd.arg("serve").env("OLLAMA_HOST", "127.0.0.1:11434");

    #[cfg(target_os = "macos")]
    {
        let current_path = std::env::var("PATH").unwrap_or_default();
        cmd.env(
            "PATH",
            format!(
                "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/Applications/Ollama.app/Contents/Resources:{}",
                current_path
            ),
        );
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start Ollama: {}", e))?;

    // Ollama normally writes server logs to stderr.
    if let Some(stderr) = child.stderr.take() {
        let app_handle = app.clone();

        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);

            for line in reader.lines() {
                let Ok(line) = line else {
                    break;
                };

                println!("[Ollama] {}", line);

                if let Some((processed, total)) = parse_prompt_processing_progress(&line) {
                    let percent = (processed as f64 / total as f64) * 100.0;

                    let payload = AnalysisProgress {
                        status: "Processing book context".to_string(),
                        processed: Some(processed),
                        total: Some(total),
                        percent: Some(percent),
                    };

                    let _ = app_handle.emit("analysis-progress", payload);
                }
            }
        });
    }

    // Also listen to stdout in case a future Ollama version uses it.
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);

            for line in reader.lines() {
                if let Ok(line) = line {
                    println!("[Ollama] {}", line);
                }
            }
        });
    }

    *guard = Some(child);

    Ok(())
}

#[tauri::command]
async fn list_local_models() -> Result<Vec<OllamaModelInfo>, String> {
    let client = reqwest::Client::new();
    let res = client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .map_err(|e| format!("Cannot connect to Ollama: {}", e))?;

    let body: OllamaListResponse = res.json().await.map_err(|e| e.to_string())?;
    Ok(body.models)
}

#[tauri::command]
async fn delete_ollama_model(name: String) -> Result<(), String> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({ "name": name });
    let res = client
        .delete("http://localhost:11434/api/delete")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Failed to delete model: {}", res.status()))
    }
}

#[tauri::command]
async fn pull_model(name: String, app: tauri::AppHandle) -> Result<(), String> {
    let client = reqwest::Client::new();
    client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .map_err(|e| format!("Ollama not reachable: {}", e))?;

    tauri::async_runtime::spawn(async move {
        let download_result = async {
            let req_body = OllamaPullRequest {
                name: name.clone(),
                stream: true,
            };

            let client = reqwest::Client::new();
            let res = client
                .post("http://localhost:11434/api/pull")
                .json(&req_body)
                .send()
                .await
                .map_err(|e| e.to_string())?;

            use futures_util::StreamExt;
            let mut byte_stream = res.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk) = byte_stream.next().await {
                let chunk = chunk.map_err(|e| e.to_string())?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Process complete lines ending in \n
                while let Some(newline_idx) = buffer.find('\n') {
                    let line = buffer[..newline_idx].trim().to_string();
                    buffer.drain(..=newline_idx);

                    if line.is_empty() {
                        continue;
                    }

                    if let Ok(progress) = serde_json::from_str::<OllamaPullProgress>(&line) {
                        let percent = match (progress.completed, progress.total) {
                            (Some(c), Some(t)) if t > 0 => Some((c as f64 / t as f64) * 100.0),
                            _ => None,
                        };
                        let payload = PullProgress {
                            status: progress.status,
                            completed: progress.completed,
                            total: progress.total,
                            percent,
                        };
                        let _ = app.emit("model-pull-progress", &payload);
                    }
                }
            }
            Ok::<_, String>(())
        }
        .await;

        match download_result {
            Ok(_) => {
                let _ = app.emit("model-pull-complete", &name);
            }
            Err(e) => {
                let _ = app.emit("model-pull-error", &e);
            }
        }
    });

    Ok(())
}

#[tauri::command]
async fn get_model_config(
    state: tauri::State<'_, ModelConfigState>,
) -> Result<ModelConfig, String> {
    let config = state.config.lock().unwrap();
    Ok(ModelConfig {
        llm_model: config.llm_model.clone(),
        embed_model: config.embed_model.clone(),
    })
}

#[tauri::command]
async fn set_model_config(
    llm_model: String,
    embed_model: String,
    state: tauri::State<'_, ModelConfigState>,
) -> Result<(), String> {
    {
        let mut config = state.config.lock().unwrap();
        config.llm_model = llm_model;
        config.embed_model = embed_model;
    }
    state.save();
    Ok(())
}

// ─── Greet ────────────────────────────────────────────────────────────────────

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// ─── Upload ───────────────────────────────────────────────────────────────────

#[tauri::command]
async fn upload_pdf(
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

// ─── Search ───────────────────────────────────────────────────────────────────

async fn embed_with_model(text: &str, model: &str) -> Result<Vec<f32>, String> {
    #[derive(serde::Serialize)]
    struct EmbedReq<'a> {
        model: &'a str,
        prompt: &'a str,
    }
    #[derive(serde::Deserialize)]
    struct EmbedResp {
        embedding: Vec<f32>,
    }

    let client = reqwest::Client::new();
    let res = client
        .post("http://localhost:11434/api/embeddings")
        .json(&EmbedReq {
            model,
            prompt: text,
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let body: EmbedResp = res.json().await.map_err(|e| e.to_string())?;
    Ok(body.embedding)
}

#[tauri::command]
async fn search_context(
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

// ─── AI Generation ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[derive(Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
}

async fn call_ollama(model: &str, prompt: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    let res = client
        .post("http://localhost:11434/api/generate")
        .json(&OllamaGenerateRequest {
            model: model.to_string(),
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
async fn generate_response(
    query: String,
    book_id: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
    storage: tauri::State<'_, AppStorage>,
    model_config: tauri::State<'_, ModelConfigState>,
) -> Result<String, String> {
    let llm_model = {
        let cfg = model_config.config.lock().unwrap();
        cfg.llm_model.clone()
    };

    let context_results =
        search_context(query.clone(), book_id, pool, storage, model_config).await?;
    let context = context_results.join("\n\n");

    let prompt = format!(
        "Use the following pieces of retrieved context to answer the user's question. \
        If you don't know the answer based on the context, just say that you don't know, \
        don't try to make up an answer. Keep the answer concise and relevant.\n\n\
        Context:\n{}\n\nQuestion: {}\n\nAnswer:",
        context, query
    );

    call_ollama(&llm_model, prompt).await
}

#[tauri::command]
async fn generate_learning_path(
    book_id: String,
    pool: tauri::State<'_, SqlitePool>,
    model_config: tauri::State<'_, ModelConfigState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let existing =
        sqlx::query_as::<_, (String,)>("SELECT content FROM learning_paths WHERE book_id = ?")
            .bind(&book_id)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    if let Some(path) = existing {
        return Ok(path.0);
    }

    let llm_model = {
        let cfg = model_config.config.lock().unwrap();
        cfg.llm_model.clone()
    };

    let chunks = sqlx::query_as::<_, (String,)>(
        "SELECT content FROM chunks WHERE book_id = ? ORDER BY chunk_index",
    )
    .bind(&book_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let context = chunks
        .into_iter()
        .map(|c| c.0)
        .collect::<Vec<_>>()
        .join("\n\n");
    let prompt = format!(
        r#"You are an expert concept finder and teacher.

Your task is to analyze the provided book context and extract the key concepts, principles, mental models, frameworks, strategies, and important ideas that a student should understand.

CONTEXT:
{}

REQUIREMENTS:

* Generate as many DISTINCT concepts from the provided context as possible.
* Do NOT return only one or two concepts.
* Each concept must represent a meaningfully different idea, principle, framework, or lesson.
* Do NOT combine multiple major ideas into one concept just to reduce the number of concepts.
* Cover different parts and themes of the book when possible.
* Prioritize concepts that are useful for actually understanding and applying the book.
* Avoid duplicate or highly overlapping concepts.
* Think broadly before producing the final answer.
* Each concept should teach the student something specific.
* For every concept, provide a clear and comprehensive explanation.
* For every concept, provide as many relevant excerpts from the provided context as possible.
* Only use excerpts that actually appear in the provided context. Do not invent quotations.
* If the context does not contain enough information for 8 concepts, generate as many distinct concepts as the context genuinely supports rather than inventing information.

OUTPUT FORMAT:

Return ONLY Markdown.

Do NOT return JSON.
Do NOT wrap the response in a Markdown code block.
Do NOT include an introduction or conclusion.

Use EXACTLY this structure for every concept:

## Concept 1: [Concept Title]

[Comprehensive explanation of the concept.]

### Excerpts

* "[Relevant excerpt]"
* "[Relevant excerpt]"
* "[Relevant excerpt]"

---

## Concept 2: [Concept Title]

[Comprehensive explanation of the concept.]

### Excerpts

* "[Relevant excerpt]"
* "[Relevant excerpt]"
* "[Relevant excerpt]"

---

Continue until you have identified all important distinct concepts.

Before producing the final answer, internally check:

1. Did I generate at least 3 distinct concepts when the context supports it?
2. Are the concepts meaningfully different from each other?
3. Does each concept have a useful explanation?
4. Does each concept have relevant excerpts?
5. Did I avoid inventing excerpts?
6. Did I cover different themes from the context?

CONCEPTS:"#,
        context
    );
    let _ = app.emit(
        "analysis-progress",
        AnalysisProgress {
            status: "Starting analysis...".to_string(),
            processed: None,
            total: None,
            percent: None,
        },
    );
    let generated_content = call_ollama(&llm_model, prompt).await?;
    sqlx::query("INSERT INTO learning_paths (book_id, content) VALUES (?, ?)")
        .bind(&book_id)
        .bind(&generated_content)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(generated_content)
}
#[tauri::command]
async fn get_lesson(
    concept: String,
    book_id: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Option<String>, String> {
    let lesson = sqlx::query_as::<_, (String,)>(
        "SELECT content FROM lessons WHERE book_id = ? AND concept = ?",
    )
    .bind(&book_id)
    .bind(&concept)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(lesson.map(|row| row.0))
}
#[tauri::command]
async fn generate_lesson(
    concept: String,
    book_id: String,
    pool: tauri::State<'_, SqlitePool>,
    storage: tauri::State<'_, AppStorage>,
    model_config: tauri::State<'_, ModelConfigState>,
) -> Result<String, String> {
    let existing = sqlx::query_as::<_, (String,)>(
        "SELECT content FROM lessons WHERE book_id = ? AND concept = ?",
    )
    .bind(&book_id)
    .bind(&concept)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    if let Some(lesson) = existing {
        return Ok(lesson.0);
    }

    let llm_model = {
        let cfg = model_config.config.lock().unwrap();
        cfg.llm_model.clone()
    };

    let context_results = search_context(
        concept.clone(),
        Some(book_id.clone()),
        pool.clone(),
        storage,
        model_config,
    )
    .await?;
    let context = context_results.join("\n\n");

    let prompt = format!(
        "You are an expert tutor. Using the provided context from the book, \
        explain the concept of '{}' in detail. \
        Provide a structured lesson with clear explanations and examples based on the text.\n\n\
        If you don't know the answer based on the context, just say that you don't know, \
        don't try to make up an answer. Keep the answer concise and relevant.\n\n\
        Context:\n{}\n\nLesson on {}:",
        concept, context, concept
    );

    let generated_content = call_ollama(&llm_model, prompt).await?;

    sqlx::query("INSERT INTO lessons (book_id, concept, content) VALUES (?, ?, ?)")
        .bind(&book_id)
        .bind(&concept)
        .bind(&generated_content)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(generated_content)
}

// ─── Data Access ──────────────────────────────────────────────────────────────

#[tauri::command]
async fn get_stored_learning_path(
    book_id: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Option<String>, String> {
    let row =
        sqlx::query_as::<_, (String,)>("SELECT content FROM learning_paths WHERE book_id = ?")
            .bind(&book_id)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
    Ok(row.map(|r| r.0))
}

#[tauri::command]
async fn get_stored_lessons(
    book_id: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<(String, String)>, String> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT concept, content FROM lessons WHERE book_id = ?",
    )
    .bind(&book_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows)
}

#[tauri::command]
async fn get_chunks(
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
async fn get_books(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<Book>, String> {
    sqlx::query_as::<_, Book>("SELECT id, title FROM books")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_book(book_id: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
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
fn parse_prompt_processing_progress(line: &str) -> Option<(u64, u64)> {
    if !line.contains("Prompt processing progress") {
        return None;
    }

    let processed = line
        .split("processed=")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;

    let total = line
        .split("total=")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;

    if total == 0 {
        return None;
    }

    Some((processed, total))
}

// ─── Database Backup / Restore ────────────────────────────────────────────────

#[tauri::command]
async fn save_database(path: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    let destination = std::path::Path::new(&path);

    // Don't overwrite an existing backup accidentally.
    if destination.exists() {
        return Err(format!(
            "Backup file already exists: {}",
            destination.display()
        ));
    }

    // Make sure the parent directory exists.
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create backup directory: {}", e))?;
    }

    // SQLite's VACUUM INTO creates a consistent snapshot of the database,
    // even while the application is using it.
    sqlx::query("VACUUM INTO ?")
        .bind(path)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Failed to save database: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn load_database(path: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    let source = std::path::Path::new(&path);

    if !source.exists() {
        return Err(format!(
            "Database backup does not exist: {}",
            source.display()
        ));
    }

    if !source.is_file() {
        return Err("Selected database backup is not a file.".to_string());
    }

    // IMPORTANT:
    // ATTACH DATABASE is connection-local, so we must use the
    // same SQLite connection for the entire restore operation.
    let mut conn = pool
        .acquire()
        .await
        .map_err(|e| format!("Failed to acquire database connection: {}", e))?;

    // Attach the backup using THIS connection.
    sqlx::query("ATTACH DATABASE ? AS backup")
        .bind(path)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to open backup database: {}", e))?;

    let result = async {
        // Verify that the backup actually contains the expected tables.
        let tables = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT name
            FROM backup.sqlite_master
            WHERE type = 'table'
              AND name NOT LIKE 'sqlite_%'
            ORDER BY name
            "#,
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| format!("Failed to inspect backup: {}", e))?;

        if tables.is_empty() {
            return Err("The selected file does not contain any application tables.".to_string());
        }

        // Check that the backup has the tables we expect.
        let required_tables = ["books", "chunks", "learning_paths", "lessons"];

        for required in required_tables {
            if !tables.iter().any(|(name,)| name == required) {
                return Err(format!(
                    "The selected backup is missing required table '{}'.",
                    required
                ));
            }
        }

        // Disable foreign-key enforcement while replacing the data.
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;

        // Use a transaction so a failure doesn't leave the database
        // partially restored.
        let mut tx = conn
            .begin()
            .await
            .map_err(|e| format!("Failed to start restore transaction: {}", e))?;

        for (table_name,) in &tables {
            let escaped = table_name.replace('"', "\"\"");

            // Clear the current table.
            let delete_sql = format!(r#"DELETE FROM main."{}""#, escaped);

            sqlx::query(&delete_sql)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to clear table '{}': {}", table_name, e))?;

            // Copy the backup table into the current database.
            let insert_sql = format!(
                r#"INSERT INTO main."{}" SELECT * FROM backup."{}""#,
                escaped, escaped
            );

            sqlx::query(&insert_sql)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to restore table '{}': {}", table_name, e))?;
        }

        tx.commit()
            .await
            .map_err(|e| format!("Failed to commit database restore: {}", e))?;

        Ok::<(), String>(())
    }
    .await;

    // Re-enable foreign keys.
    let _ = sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await;

    // IMPORTANT:
    // DETACH must also happen on the same connection that did ATTACH.
    let detach_result = sqlx::query("DETACH DATABASE backup")
        .execute(&mut *conn)
        .await;

    if let Err(e) = detach_result {
        if result.is_ok() {
            return Err(format!("Failed to detach backup database: {}", e));
        }
    }

    result
}

fn kill_existing_ollama() {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "ollama.exe", "/T"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "ollama app.exe", "/T"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("/usr/bin/pkill")
            .args(["-x", "ollama"])
            .output();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("/usr/bin/pkill")
            .args(["-x", "ollama"])
            .output();
    }
}

fn find_ollama() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        // 1. Check user local appdata (standard Ollama Windows installer)
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let p = std::path::PathBuf::from(local_app_data)
                .join("Programs")
                .join("Ollama")
                .join("ollama.exe");
            if p.exists() {
                return Ok(p.to_string_lossy().to_string());
            }
        }

        // 2. Check Program Files
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            let p = std::path::PathBuf::from(program_files)
                .join("Ollama")
                .join("ollama.exe");
            if p.exists() {
                return Ok(p.to_string_lossy().to_string());
            }
        }

        // 3. Check Program Files (x86)
        if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
            let p = std::path::PathBuf::from(program_files_x86)
                .join("Ollama")
                .join("ollama.exe");
            if p.exists() {
                return Ok(p.to_string_lossy().to_string());
            }
        }

        // 4. Search in PATH
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let candidate = dir.join("ollama.exe");
                if candidate.exists() {
                    return Ok(candidate.to_string_lossy().to_string());
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let candidates = [
            "/Applications/Ollama.app/Contents/Resources/ollama",
            "/usr/local/bin/ollama",
            "/opt/homebrew/bin/ollama",
            "/usr/bin/ollama",
        ];

        for path in candidates {
            if std::path::Path::new(path).exists() {
                return Ok(path.to_string());
            }
        }

        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let candidate = dir.join("ollama");
                if candidate.exists() {
                    return Ok(candidate.to_string_lossy().to_string());
                }
            }
        }
    }

    Err(
        "Could not find Ollama executable. Please ensure Ollama is installed and running."
            .to_string(),
    )
}
// ─── App Setup ────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Resolve canonical App Data Directory
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");

            // Ensure directory exists synchronously before initializing file paths
            std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");

            let db_path = app_data_dir.join("app.db");
            let options = sqlx::sqlite::SqliteConnectOptions::new()
                .filename(db_path.clone())
                .create_if_missing(true);

            let pool = tauri::async_runtime::block_on(db::init_db(options)).unwrap_or_else(|e| {
                panic!("failed to initialize database at {:?}: {}", db_path, e)
            });

            // Load ModelConfigState using canonical app_data_dir
            let model_config = ModelConfigState::load(&app_data_dir);

            app.manage(pool);
            app.manage(AppStorage { dir: app_data_dir });
            app.manage(model_config);
            app.manage(OllamaProcess::new());
            let ollama_process = app.state::<OllamaProcess>();

            if let Err(e) = start_ollama(app.handle().clone(), ollama_process) {
                eprintln!("Failed to start Ollama: {}", e);
            }

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
            get_stored_lessons,
            // Model management
            list_local_models,
            delete_ollama_model,
            pull_model,
            get_model_config,
            set_model_config,
            get_lesson,
            save_database,
            load_database,
            // Ollama
            start_ollama,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
