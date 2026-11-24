use crate::convert_pdf_to_markdown;
use crate::ollama::{embed_with_model, ModelConfigState};
use crate::rag::faiss::VectorIndex;
use regex::Regex;
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
use reqwest::Client;
use serde::Deserialize;

#[derive(Serialize)]
struct OllamaEmbedRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

fn split_sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0;

    for (idx, ch) in text.char_indices() {
        if matches!(ch, '.' | '!' | '?') {
            let next_idx = idx + ch.len_utf8();
            let rest = &text[next_idx..];
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                let s = text[start..next_idx].trim();
                if !s.is_empty() {
                    sentences.push(s);
                }
                start = next_idx + (rest.len() - rest.trim_start().len());
            }
        }
    }

    let tail = text[start..].trim();
    if !tail.is_empty() {
        sentences.push(tail);
    }
    sentences
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }
    1.0 - (dot / (norm_a * norm_b))
}

fn percentile_threshold(distances: &[f32], percentile: f32) -> f32 {
    if distances.is_empty() {
        return 1.0;
    }
    let mut sorted = distances.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));

    let clamped = percentile.clamp(0.0, 100.0);
    let idx =
        (((clamped / 100.0) * (sorted.len() - 1) as f32).round() as usize).min(sorted.len() - 1);
    sorted[idx]
}

fn split_long_chunk(text: &str, max_words: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= max_words || max_words == 0 {
        return vec![text.to_string()];
    }
    words.chunks(max_words).map(|w| w.join(" ")).collect()
}

/// Semantic chunking backed by local Ollama (`nomic-embed-text`).
///
/// Recommended defaults:
/// - `buffer_size`: `1` (1 sentence before + 1 after)
/// - `percentile`: `90.0` (splits on top 10% topic shifts)
/// - `max_words`: `350` (keeps chunks well inside nomic-embed-text's token window)
pub async fn semantic_chunk_ollama(
    client: &Client,
    text: &str,
    buffer_size: usize,
    percentile: f32,
    max_words: usize,
    embed_model: &str,
) -> Result<Vec<String>, reqwest::Error> {
    let sentences = split_sentences(text);
    if sentences.is_empty() {
        return Ok(Vec::new());
    }
    if sentences.len() == 1 {
        return Ok(split_long_chunk(sentences[0], max_words));
    }

    // 1. Build sliding windows with nomic document prefix
    let windows: Vec<String> = (0..sentences.len())
        .map(|i| {
            let start = i.saturating_sub(buffer_size);
            let end = usize::min(i + buffer_size + 1, sentences.len());
            format!("search_document: {}", sentences[start..end].join(" "))
        })
        .collect();

    // 2. Batch-embed all windows in one local Ollama request (chunked in batches of 64)
    let mut embeddings = Vec::with_capacity(windows.len());
    for batch in windows.chunks(64) {
        let resp = client
            .post("http://localhost:11434/api/embed")
            .json(&OllamaEmbedRequest {
                model: embed_model,
                input: batch,
            })
            .send()
            .await?
            .error_for_status()?
            .json::<OllamaEmbedResponse>()
            .await?;
        embeddings.extend(resp.embeddings);
    }

    // 3. Adjacent cosine distances & percentile breakpoint
    let distances: Vec<f32> = embeddings
        .windows(2)
        .map(|pair| cosine_distance(&pair[0], &pair[1]))
        .collect();
    let threshold = percentile_threshold(&distances, percentile);

    // 4. Group sentences & enforce max_words cap
    let mut chunks = Vec::new();
    let mut current: Vec<&str> = vec![sentences[0]];
    let mut current_words = sentences[0].split_whitespace().count();

    for (i, &dist) in distances.iter().enumerate() {
        let next_sentence = sentences[i + 1];
        let next_words = next_sentence.split_whitespace().count();

        if dist >= threshold || (max_words > 0 && current_words + next_words > max_words) {
            chunks.extend(split_long_chunk(&current.join(" "), max_words));
            current = vec![next_sentence];
            current_words = next_words;
        } else {
            current.push(next_sentence);
            current_words += next_words;
        }
    }

    if !current.is_empty() {
        chunks.extend(split_long_chunk(&current.join(" "), max_words));
    }

    Ok(chunks)
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

fn clear_markdown_formatting(markdown: &str) -> String {
    // 1. Remove ***, ###, and multi-line code blocks inside ``` ```
    let md_re = Regex::new(r"(?s)\*\*\*|###|```.*?```").unwrap();
    let cleaned = md_re.replace_all(markdown, "");

    // 2. Remove clusters of 3 or more 1-to-2 letter gibberish tokens in a row
    // e.g., matches "s d s se" or "s s s s a e" without deleting normal words like "is a"
    let gibberish_re = Regex::new(r"\b[a-zA-Z]{1,2}(?:\s+[a-zA-Z]{1,2}){2,}\b").unwrap();
    let cleaned = gibberish_re.replace_all(&cleaned, "");

    // 3. Clean up any double spaces left behind
    let space_re = Regex::new(r" {2,}").unwrap();
    space_re.replace_all(cleaned.trim(), " ").to_string()
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

    // let text = pdf_extract::extract_text(&path).map_err(|e| e.to_string())?;

    let text = convert_pdf_to_markdown(path.clone())
        .await
        .map_err(|e| e.to_string())?;

    let text = clear_markdown_formatting(&text);

    if text.trim().is_empty() {
        return Err("No text extracted. PDF might be scanned/image-only.".into());
    }

    sqlx::query("INSERT INTO books (id, title, file_path) VALUES (?, ?, ?)")
        .bind(book_id.clone())
        .bind(title)
        .bind(path)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    let embed_model = {
        let cfg = model_config.config.lock().unwrap();
        cfg.embed_model.clone()
    };

    let client = Client::new();
    let chunks = semantic_chunk_ollama(&client, &text, 1, 90.0, 350, &embed_model)
        .await
        .map_err(|e| e.to_string())?;

    let mut index = load_or_create_faiss(&storage.dir);

    for (i, chunk) in chunks.iter().enumerate() {
        let embedding = embed_with_model(chunk, &embed_model).await?;

        index.add(&embedding);
        let faiss_id = (index.ntotal() - 1) as i64;

        sqlx::query(
            "INSERT INTO chunks (book_id, chunk_index, content, faiss_id)
             VALUES (?, ?, ?, ?)",
        )
        .bind(book_id.clone())
        .bind(i as i32)
        .bind(chunk)
        .bind(faiss_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
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

    // 1. Fetch chunks belonging to the targeted book (or all chunks if no book selected)
    let chunks: Vec<(i64, String)> = if let Some(ref bid) = book_id {
        sqlx::query_as::<_, (i64, String)>("SELECT faiss_id, content FROM chunks WHERE book_id = ?")
            .bind(bid)
            .fetch_all(pool.inner())
            .await
            .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as::<_, (i64, String)>("SELECT faiss_id, content FROM chunks")
            .fetch_all(pool.inner())
            .await
            .map_err(|e| e.to_string())?
    };

    if chunks.is_empty() {
        return Ok(Vec::new());
    }

    let chunk_map: std::collections::HashMap<i64, String> = chunks.into_iter().collect();
    let candidate_ids: Vec<u64> = chunk_map.keys().map(|&id| id as u64).collect();

    let mut results = Vec::new();

    // 2. Perform vector search scoped specifically to the candidate chunks
    if let Ok(query_embedding) = embed_with_model(&query, &embed_model).await {
        let index = load_or_create_faiss(&storage.dir);
        let ids = index.search_with_candidates(&query_embedding, &candidate_ids, 20);
        for id in ids {
            if let Some(content) = chunk_map.get(&(id as i64)) {
                results.push(content.clone());
            }
        }
    }

    // 3. Fallback: Lexical / keyword search if vector search found 0 passages
    if results.is_empty() {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() >= 2)
            .collect();

        let mut scored_chunks: Vec<(&String, usize)> = chunk_map
            .values()
            .filter_map(|content| {
                let content_lower = content.to_lowercase();
                let mut score = 0;
                if content_lower.contains(&query_lower) {
                    score += 100;
                }
                for word in &query_words {
                    if content_lower.contains(word) {
                        score += 10;
                    }
                }
                if score > 0 {
                    Some((content, score))
                } else {
                    None
                }
            })
            .collect();

        scored_chunks.sort_by(|a, b| b.1.cmp(&a.1));
        for (content, _) in scored_chunks.into_iter().take(5) {
            results.push(content.clone());
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

pub async fn purge_ghost_vectors(
    pool: &SqlitePool,
    storage_dir: &std::path::Path,
) -> Result<(), String> {
    let mut index = load_or_create_faiss(storage_dir);
    if index.ntotal() == 0 {
        return Ok(());
    }

    // Fetch all current active chunks ordered by id
    let active_chunks: Vec<(i32, i64)> =
        sqlx::query_as::<_, (i32, i64)>("SELECT id, faiss_id FROM chunks ORDER BY id ASC")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

    if active_chunks.is_empty() {
        index.clear();
        index.save(storage_dir.join("faiss.index").to_str().unwrap());
        return Ok(());
    }

    let mut new_vectors = Vec::with_capacity(active_chunks.len());
    let mut id_updates: Vec<(i32, i64)> = Vec::new();

    for (chunk_id, old_faiss_id) in active_chunks {
        if let Some(vec) = index.get_vector(old_faiss_id as usize) {
            let new_faiss_id = new_vectors.len() as i64;
            new_vectors.push(vec.clone());
            if new_faiss_id != old_faiss_id {
                id_updates.push((chunk_id, new_faiss_id));
            }
        }
    }

    if !id_updates.is_empty() {
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
        for (chunk_id, new_faiss_id) in id_updates {
            sqlx::query("UPDATE chunks SET faiss_id = ? WHERE id = ?")
                .bind(new_faiss_id)
                .bind(chunk_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        tx.commit().await.map_err(|e| e.to_string())?;
    }

    index.set_vectors(new_vectors);
    index.save(storage_dir.join("faiss.index").to_str().unwrap());

    Ok(())
}

#[tauri::command]
pub async fn delete_book(
    book_id: String,
    pool: tauri::State<'_, SqlitePool>,
    storage: tauri::State<'_, AppStorage>,
) -> Result<(), String> {
    let _ = sqlx::query("DELETE FROM lessons WHERE book_id = ?")
        .bind(&book_id)
        .execute(pool.inner())
        .await;

    let _ = sqlx::query("DELETE FROM learning_paths WHERE book_id = ?")
        .bind(&book_id)
        .execute(pool.inner())
        .await;

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

    purge_ghost_vectors(pool.inner(), &storage.dir).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn test_purge_ghost_vectors() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                book_id TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                faiss_id INTEGER NOT NULL
            );",
        )
        .execute(&pool)
        .await
        .unwrap();

        let temp_dir = std::env::temp_dir().join(format!("test_purge_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut index = VectorIndex::new(3);
        index.add(&[1.0, 0.0, 0.0]); // faiss_id 0 (Book 1)
        index.add(&[0.8, 0.2, 0.0]); // faiss_id 1 (Book 1)
        index.add(&[0.0, 1.0, 0.0]); // faiss_id 2 (Book 2)
        index.add(&[0.0, 0.9, 0.1]); // faiss_id 3 (Book 2)
        index.save(temp_dir.join("faiss.index").to_str().unwrap());

        // Insert chunks for Book 1
        sqlx::query(
            "INSERT INTO chunks (book_id, chunk_index, content, faiss_id) VALUES (?, ?, ?, ?)",
        )
        .bind("book_1")
        .bind(0)
        .bind("chunk 0")
        .bind(0)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chunks (book_id, chunk_index, content, faiss_id) VALUES (?, ?, ?, ?)",
        )
        .bind("book_1")
        .bind(1)
        .bind("chunk 1")
        .bind(1)
        .execute(&pool)
        .await
        .unwrap();

        // Insert chunks for Book 2
        sqlx::query(
            "INSERT INTO chunks (book_id, chunk_index, content, faiss_id) VALUES (?, ?, ?, ?)",
        )
        .bind("book_2")
        .bind(0)
        .bind("chunk 2")
        .bind(2)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chunks (book_id, chunk_index, content, faiss_id) VALUES (?, ?, ?, ?)",
        )
        .bind("book_2")
        .bind(1)
        .bind("chunk 3")
        .bind(3)
        .execute(&pool)
        .await
        .unwrap();

        // Delete Book 1 from SQLite
        sqlx::query("DELETE FROM chunks WHERE book_id = 'book_1'")
            .execute(&pool)
            .await
            .unwrap();

        // Purge ghost vectors
        purge_ghost_vectors(&pool, &temp_dir).await.unwrap();

        // Check compacted vector index
        let loaded_index = load_or_create_faiss(&temp_dir);
        assert_eq!(loaded_index.ntotal(), 2);

        // Check that Book 2 chunks now have faiss_id 0 and 1 in SQLite
        let remaining: Vec<(i32, i64, String)> =
            sqlx::query_as("SELECT id, faiss_id, content FROM chunks ORDER BY id ASC")
                .fetch_all(&pool)
                .await
                .unwrap();

        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].1, 0); // new faiss_id for chunk 2
        assert_eq!(remaining[1].1, 1); // new faiss_id for chunk 3

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
