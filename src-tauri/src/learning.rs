use crate::books::{search_context, AppStorage};
use crate::ollama::{call_ollama, AnalysisProgress, ModelConfigState};
use sqlx::SqlitePool;
use tauri::Emitter;

#[tauri::command]
pub async fn generate_response(
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
pub async fn generate_learning_path(
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
        r#"You are an expert concept and knowledge finder and teacher.

Your task is to analyze the provided book context and extract the key concepts,knowledge, principles, mental models, frameworks, strategies, and important ideas that a student should understand.

CONTEXT:
{}

REQUIREMENTS:

* Generate as many DISTINCT concepts from the provided context as possible.
* Do NOT return only one or two concepts.
* Each concept must represent a meaningfull piece of knowledge, idea, principle, framework, or lesson.
* Do NOT combine multiple major ideas into one concept just to reduce the number of concepts.
* Cover different parts and themes of the book when possible.
* For every concept, piece of knowledge, provide a clear and comprehensive explanation.
* For every concept, piece of knowledge, provide as many relevant excerpts from the provided context as possible.
* Only use excerpts that actually appear in the provided context. Do not invent quotations.

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

Continue until you have identified all important distinct concepts, knowledge, principles, frameworks, and lessons.

Before producing the final answer, internally check:

1. Did I generate as many as possible (10-20) distinct concepts, knowledge, principles, frameworks, and lessons when the context supports it?
2. Are the concepts, knowledge, principles, frameworks, and lessons meaningfully different from each other?
3. Does each concept, knowledge, principle, framework, and lesson have a useful explanation?
4. Does each concept, knowledge, principle, framework, and lesson have relevant excerpts?
5. Did I avoid inventing excerpts?

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
pub async fn get_lesson(
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
pub async fn generate_lesson(
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

#[tauri::command]
pub async fn get_stored_learning_path(
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
pub async fn get_stored_lessons(
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

#[derive(serde::Deserialize)]
pub struct NoteFile {
    pub title: String,
    pub content: String,
}

#[derive(serde::Serialize)]
pub struct ExportNotesResult {
    pub count: usize,
    pub folder_path: String,
}

#[tauri::command]
pub async fn export_notes(
    destination_dir: String,
    book_title: String,
    notes: Vec<NoteFile>,
) -> Result<ExportNotesResult, String> {
    if notes.is_empty() {
        return Err("No notes to export".to_string());
    }

    let base_path = std::path::PathBuf::from(&destination_dir);
    if !base_path.exists() {
        return Err(format!("Destination directory does not exist: {}", destination_dir));
    }

    let sanitized_book_title = sanitize_filename(&book_title);
    let subfolder_name = if sanitized_book_title.is_empty() {
        "Book Notes".to_string()
    } else {
        format!("{} - Notes", sanitized_book_title)
    };

    // If base_path already ends with subfolder_name, write directly there
    let target_dir = if base_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.eq_ignore_ascii_case(&subfolder_name))
        .unwrap_or(false)
    {
        base_path
    } else {
        base_path.join(&subfolder_name)
    };

    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create directory {:?}: {}", target_dir, e))?;

    let mut written_count = 0;
    let mut used_names = std::collections::HashSet::new();

    for note in notes {
        let base_name = sanitize_filename(&note.title);
        let safe_title = if base_name.is_empty() {
            "Untitled Concept".to_string()
        } else {
            base_name
        };

        let mut final_name = format!("{}.md", safe_title);
        let mut counter = 1;
        while used_names.contains(&final_name.to_lowercase()) {
            final_name = format!("{} ({}).md", safe_title, counter);
            counter += 1;
        }
        used_names.insert(final_name.to_lowercase());

        let file_path = target_dir.join(&final_name);
        std::fs::write(&file_path, note.content.as_bytes())
            .map_err(|e| format!("Failed to write note {:?}: {}", file_path, e))?;

        written_count += 1;
    }

    Ok(ExportNotesResult {
        count: written_count,
        folder_path: target_dir.to_string_lossy().to_string(),
    })
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            other => other,
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string()
}

