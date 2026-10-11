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
