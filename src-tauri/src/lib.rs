pub mod backup;
pub mod books;
pub mod database;
pub mod learning;
pub mod ollama;
pub mod rag;
pub mod sqlx_db;

// Re-exports for backup.rs and external consumers
pub use books::AppStorage;
pub use ollama::{ModelConfig, ModelConfigState, OllamaProcess};

use books::{delete_book, get_books, get_chunks, search_context, upload_pdf};
use database::{load_database, save_database};
use learning::{
    generate_learning_path, generate_lesson, generate_response, get_lesson,
    get_stored_learning_path, get_stored_lessons,
};
use ollama::{
    delete_ollama_model, get_model_config, list_local_models, pull_model, set_model_config,
    start_ollama,
};
use tauri::Manager;

// ─── Greet ────────────────────────────────────────────────────────────────────

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
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

            let pool =
                tauri::async_runtime::block_on(sqlx_db::init_db(options)).unwrap_or_else(|e| {
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
            // Backup / restore
            backup::save_backup,
            backup::load_backup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
