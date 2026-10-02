use crate::{AppStorage, ModelConfig, ModelConfigState};

use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, State};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const BACKUP_FORMAT_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupBook {
    id: String,
    title: String,
    pdf_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupManifest {
    format_version: u32,
    books: Vec<BackupBook>,
}

fn error_string<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn unique_staging_dir(base: &Path, prefix: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    base.join(format!(".{}_{}_{}", prefix, std::process::id(), timestamp))
}

fn sql_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn sql_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn safe_pdf_extension(path: &Path) -> String {
    path.extension()
        .and_then(|x| x.to_str())
        .map(|x| x.to_ascii_lowercase())
        .filter(|x| !x.is_empty())
        .map(|x| format!(".{}", x))
        .unwrap_or_else(|| ".pdf".to_string())
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), String> {
    if !source.exists() {
        return Err(format!("File does not exist: {}", source.display()));
    }

    if !source.is_file() {
        return Err(format!("Path is not a file: {}", source.display()));
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(error_string)?;
    }

    fs::copy(source, destination).map_err(error_string)?;

    Ok(())
}

fn cleanup(path: &Path) {
    if path.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else if path.exists() {
        let _ = fs::remove_file(path);
    }
}

async fn create_database_snapshot(pool: &SqlitePool, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        fs::remove_file(destination).map_err(error_string)?;
    }

    let destination_str = destination
        .to_str()
        .ok_or_else(|| "Invalid database path".to_string())?;

    let sql = format!("VACUUM INTO {}", sql_string(destination_str));

    sqlx::query(&sql)
        .execute(pool)
        .await
        .map_err(error_string)?;

    if !destination.exists() {
        return Err("SQLite did not create the database snapshot.".to_string());
    }

    Ok(())
}

fn add_file_to_zip(
    zip: &mut ZipWriter<File>,
    source: &Path,
    archive_path: &str,
    options: SimpleFileOptions,
) -> Result<(), String> {
    let mut input = File::open(source).map_err(error_string)?;

    zip.start_file(archive_path, options)
        .map_err(error_string)?;

    io::copy(&mut input, zip).map_err(error_string)?;

    Ok(())
}

fn create_backup_zip(
    staging: &Path,
    destination: &Path,
    manifest: &BackupManifest,
) -> Result<(), String> {
    if destination.exists() {
        fs::remove_file(destination).map_err(error_string)?;
    }

    let file = File::create(destination).map_err(error_string)?;

    let mut zip = ZipWriter::new(file);

    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let manifest_json = serde_json::to_vec_pretty(manifest).map_err(error_string)?;

    zip.start_file("manifest.json", options)
        .map_err(error_string)?;

    zip.write_all(&manifest_json).map_err(error_string)?;

    add_file_to_zip(
        &mut zip,
        &staging.join("database/app.db"),
        "database/app.db",
        options,
    )?;

    add_file_to_zip(
        &mut zip,
        &staging.join("faiss/faiss.index"),
        "faiss/faiss.index",
        options,
    )?;

    add_file_to_zip(
        &mut zip,
        &staging.join("config/model_config.json"),
        "config/model_config.json",
        options,
    )?;

    for book in &manifest.books {
        let source = staging.join(&book.pdf_path);

        add_file_to_zip(&mut zip, &source, &book.pdf_path, options)?;
    }

    zip.finish().map_err(error_string)?;

    Ok(())
}

fn validate_zip_path(path: &str) -> Result<(), String> {
    let p = Path::new(path);

    if p.is_absolute() {
        return Err(format!("Unsafe absolute path in backup: {}", path));
    }

    for component in p.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err(format!("Unsafe path in backup: {}", path));
            }

            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(format!("Unsafe path in backup: {}", path));
            }

            _ => {}
        }
    }

    Ok(())
}

fn extract_zip_file(
    archive: &mut ZipArchive<File>,
    archive_name: &str,
    destination: &Path,
) -> Result<(), String> {
    validate_zip_path(archive_name)?;

    let mut entry = archive
        .by_name(archive_name)
        .map_err(|_| format!("Backup is missing required file '{}'", archive_name))?;

    if entry.is_dir() {
        return Err(format!("Backup entry '{}' is a directory", archive_name));
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(error_string)?;
    }

    let mut output = File::create(destination).map_err(error_string)?;

    io::copy(&mut entry, &mut output).map_err(error_string)?;

    Ok(())
}

fn extract_backup(zip_path: &Path, staging: &Path) -> Result<BackupManifest, String> {
    let file = File::open(zip_path).map_err(error_string)?;

    let mut archive = ZipArchive::new(file).map_err(error_string)?;

    extract_zip_file(
        &mut archive,
        "manifest.json",
        &staging.join("manifest.json"),
    )?;

    let manifest_text = fs::read_to_string(staging.join("manifest.json")).map_err(error_string)?;

    let manifest: BackupManifest = serde_json::from_str(&manifest_text)
        .map_err(|e| format!("Invalid backup manifest: {}", e))?;

    if manifest.format_version != BACKUP_FORMAT_VERSION {
        return Err(format!(
            "Unsupported backup format version {}. Expected {}.",
            manifest.format_version, BACKUP_FORMAT_VERSION
        ));
    }

    extract_zip_file(
        &mut archive,
        "database/app.db",
        &staging.join("database/app.db"),
    )?;

    extract_zip_file(
        &mut archive,
        "faiss/faiss.index",
        &staging.join("faiss/faiss.index"),
    )?;

    extract_zip_file(
        &mut archive,
        "config/model_config.json",
        &staging.join("config/model_config.json"),
    )?;

    for book in &manifest.books {
        validate_zip_path(&book.pdf_path)?;

        if !book.pdf_path.starts_with("pdfs/") {
            return Err(format!("Invalid PDF path in manifest: {}", book.pdf_path));
        }

        extract_zip_file(&mut archive, &book.pdf_path, &staging.join(&book.pdf_path))?;
    }

    Ok(manifest)
}

/*
 * Restores the SQLite schema itself.
 *
 * This is important for your previous error:
 *
 *     table main.books has 4 columns but 5 values were supplied
 *
 * We don't copy rows into the old schema.
 * We recreate the schema from the backup.
 */
async fn restore_sqlite_database(pool: &SqlitePool, backup_db: &Path) -> Result<(), String> {
    let mut conn = pool.acquire().await.map_err(error_string)?;

    let backup_path = backup_db
        .to_str()
        .ok_or_else(|| "Invalid backup database path".to_string())?;

    let attach_sql = format!("ATTACH DATABASE {} AS backup", sql_string(backup_path));

    sqlx::query(&attach_sql)
        .execute(&mut *conn)
        .await
        .map_err(error_string)?;

    let result = restore_attached_database(&mut conn).await;

    let detach_result = sqlx::query("DETACH DATABASE backup")
        .execute(&mut *conn)
        .await;

    if let Err(error) = detach_result {
        if result.is_ok() {
            return Err(format!("Failed to detach backup database: {}", error));
        }
    }

    result
}

async fn restore_attached_database(conn: &mut SqliteConnection) -> Result<(), String> {
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await
        .map_err(error_string)?;

    let result = async {
        sqlx::query("BEGIN EXCLUSIVE")
            .execute(&mut *conn)
            .await
            .map_err(error_string)?;

        let operation = async {
            /*
             * Drop views.
             */
            let views: Vec<String> = sqlx::query_scalar(
                r#"
                SELECT name
                FROM main.sqlite_master
                WHERE type = 'view'
                  AND name NOT LIKE 'sqlite_%'
                ORDER BY name
                "#,
            )
            .fetch_all(&mut *conn)
            .await
            .map_err(error_string)?;

            for name in views {
                let sql = format!("DROP VIEW IF EXISTS main.{}", sql_identifier(&name));

                sqlx::query(&sql)
                    .execute(&mut *conn)
                    .await
                    .map_err(error_string)?;
            }

            /*
             * Drop triggers.
             */
            let triggers: Vec<String> = sqlx::query_scalar(
                r#"
                SELECT name
                FROM main.sqlite_master
                WHERE type = 'trigger'
                  AND name NOT LIKE 'sqlite_%'
                ORDER BY name
                "#,
            )
            .fetch_all(&mut *conn)
            .await
            .map_err(error_string)?;

            for name in triggers {
                let sql = format!("DROP TRIGGER IF EXISTS main.{}", sql_identifier(&name));

                sqlx::query(&sql)
                    .execute(&mut *conn)
                    .await
                    .map_err(error_string)?;
            }

            /*
             * Drop indexes.
             */
            let indexes: Vec<String> = sqlx::query_scalar(
                r#"
                SELECT name
                FROM main.sqlite_master
                WHERE type = 'index'
                  AND name NOT LIKE 'sqlite_%'
                ORDER BY name
                "#,
            )
            .fetch_all(&mut *conn)
            .await
            .map_err(error_string)?;

            for name in indexes {
                let sql = format!("DROP INDEX IF EXISTS main.{}", sql_identifier(&name));

                sqlx::query(&sql)
                    .execute(&mut *conn)
                    .await
                    .map_err(error_string)?;
            }

            /*
             * Drop tables.
             */
            let tables: Vec<String> = sqlx::query_scalar(
                r#"
                SELECT name
                FROM main.sqlite_master
                WHERE type = 'table'
                  AND name NOT LIKE 'sqlite_%'
                ORDER BY name
                "#,
            )
            .fetch_all(&mut *conn)
            .await
            .map_err(error_string)?;

            for name in tables {
                let sql = format!("DROP TABLE IF EXISTS main.{}", sql_identifier(&name));

                sqlx::query(&sql)
                    .execute(&mut *conn)
                    .await
                    .map_err(error_string)?;
            }

            /*
             * Recreate tables from backup.
             */
            let tables: Vec<(String, String)> = sqlx::query_as(
                r#"
                    SELECT name, sql
                    FROM backup.sqlite_master
                    WHERE type = 'table'
                      AND name NOT LIKE 'sqlite_%'
                      AND sql IS NOT NULL
                    ORDER BY name
                    "#,
            )
            .fetch_all(&mut *conn)
            .await
            .map_err(error_string)?;

            for (_, create_sql) in &tables {
                sqlx::query(create_sql)
                    .execute(&mut *conn)
                    .await
                    .map_err(error_string)?;
            }

            /*
             * Copy table data using explicit columns.
             */
            for (table_name, _) in &tables {
                let pragma = format!("PRAGMA backup.table_info({})", sql_identifier(table_name));

                let columns: Vec<(i64, String, String, i64, Option<String>, i64)> =
                    sqlx::query_as(&pragma)
                        .fetch_all(&mut *conn)
                        .await
                        .map_err(error_string)?;

                if columns.is_empty() {
                    continue;
                }

                let column_list = columns
                    .iter()
                    .map(|(_, name, _, _, _, _)| sql_identifier(name))
                    .collect::<Vec<_>>()
                    .join(", ");

                let table = sql_identifier(table_name);

                let sql = format!(
                    "INSERT INTO main.{table} ({columns})
                     SELECT {columns}
                     FROM backup.{table}",
                    table = table,
                    columns = column_list,
                );

                sqlx::query(&sql)
                    .execute(&mut *conn)
                    .await
                    .map_err(error_string)?;
            }

            /*
             * Restore sqlite_sequence when applicable.
             */
            let sequence_exists: (i64,) = sqlx::query_as(
                r#"
                    SELECT COUNT(*)
                    FROM backup.sqlite_master
                    WHERE type = 'table'
                      AND name = 'sqlite_sequence'
                    "#,
            )
            .fetch_one(&mut *conn)
            .await
            .map_err(error_string)?;

            if sequence_exists.0 > 0 {
                let main_sequence_exists: (i64,) = sqlx::query_as(
                    r#"
                        SELECT COUNT(*)
                        FROM main.sqlite_master
                        WHERE type = 'table'
                          AND name = 'sqlite_sequence'
                        "#,
                )
                .fetch_one(&mut *conn)
                .await
                .map_err(error_string)?;

                if main_sequence_exists.0 > 0 {
                    sqlx::query("DELETE FROM main.sqlite_sequence")
                        .execute(&mut *conn)
                        .await
                        .map_err(error_string)?;

                    sqlx::query(
                        "INSERT INTO main.sqlite_sequence
                         SELECT * FROM backup.sqlite_sequence",
                    )
                    .execute(&mut *conn)
                    .await
                    .map_err(error_string)?;
                }
            }

            /*
             * Recreate indexes, triggers and views.
             */
            let objects: Vec<(String, String, String)> = sqlx::query_as(
                r#"
                    SELECT type, name, sql
                    FROM backup.sqlite_master
                    WHERE type IN ('index', 'trigger', 'view')
                      AND name NOT LIKE 'sqlite_%'
                      AND sql IS NOT NULL
                    ORDER BY
                        CASE type
                            WHEN 'index' THEN 1
                            WHEN 'trigger' THEN 2
                            WHEN 'view' THEN 3
                            ELSE 4
                        END,
                        name
                    "#,
            )
            .fetch_all(&mut *conn)
            .await
            .map_err(error_string)?;

            for (_, _, create_sql) in objects {
                sqlx::query(&create_sql)
                    .execute(&mut *conn)
                    .await
                    .map_err(error_string)?;
            }

            /*
             * Restore PRAGMA user_version.
             */
            let user_version: (i64,) = sqlx::query_as("PRAGMA backup.user_version")
                .fetch_one(&mut *conn)
                .await
                .map_err(error_string)?;

            sqlx::query(&format!("PRAGMA main.user_version = {}", user_version.0))
                .execute(&mut *conn)
                .await
                .map_err(error_string)?;

            Ok::<(), String>(())
        }
        .await;

        match operation {
            Ok(()) => {
                sqlx::query("COMMIT")
                    .execute(&mut *conn)
                    .await
                    .map_err(error_string)?;

                Ok(())
            }

            Err(error) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;

                Err(error)
            }
        }
    }
    .await;

    let _ = sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await;

    result
}

/* ─────────────────────────────────────────────────────────────
SAVE
───────────────────────────────────────────────────────────── */

#[tauri::command]
pub async fn save_backup(
    path: String,
    pool: State<'_, SqlitePool>,
    storage: State<'_, AppStorage>,
    model_config: State<'_, ModelConfigState>,
) -> Result<(), String> {
    let destination = PathBuf::from(path);

    if destination
        .extension()
        .and_then(|x| x.to_str())
        .map(|x| x.eq_ignore_ascii_case("zip"))
        != Some(true)
    {
        return Err("Backup file must have a .zip extension.".to_string());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(error_string)?;
    }

    let staging = unique_staging_dir(&storage.dir, "backup-save");

    fs::create_dir_all(staging.join("database")).map_err(error_string)?;

    fs::create_dir_all(staging.join("faiss")).map_err(error_string)?;

    fs::create_dir_all(staging.join("config")).map_err(error_string)?;

    fs::create_dir_all(staging.join("pdfs")).map_err(error_string)?;

    let result = async {
        /*
         * Snapshot SQLite.
         */
        create_database_snapshot(pool.inner(), &staging.join("database/app.db")).await?;

        /*
         * Save current model configuration first.
         */
        model_config.save();

        copy_file(
            &storage.dir.join("faiss.index"),
            &staging.join("faiss/faiss.index"),
        )?;

        copy_file(
            &storage.dir.join("model_config.json"),
            &staging.join("config/model_config.json"),
        )?;

        /*
         * Get all books and their current PDF paths.
         */
        let books: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, title, file_path FROM books ORDER BY id")
                .fetch_all(pool.inner())
                .await
                .map_err(error_string)?;

        let mut manifest_books = Vec::new();

        for (book_id, title, file_path) in books {
            let source = PathBuf::from(&file_path);

            if !source.exists() {
                return Err(format!(
                    "Cannot create backup because PDF for '{}' does not exist:\n{}",
                    title,
                    source.display()
                ));
            }

            if !source.is_file() {
                return Err(format!(
                    "PDF path for '{}' is not a file:\n{}",
                    title,
                    source.display()
                ));
            }

            let extension = safe_pdf_extension(&source);

            let archive_path = format!("pdfs/{}{}", book_id, extension);

            copy_file(&source, &staging.join(&archive_path))?;

            manifest_books.push(BackupBook {
                id: book_id,
                title,
                pdf_path: archive_path,
            });
        }

        let manifest = BackupManifest {
            format_version: BACKUP_FORMAT_VERSION,
            books: manifest_books,
        };

        create_backup_zip(&staging, &destination, &manifest)?;

        Ok::<(), String>(())
    }
    .await;

    cleanup(&staging);

    result
}

/* ─────────────────────────────────────────────────────────────
LOAD
───────────────────────────────────────────────────────────── */

#[tauri::command]
pub async fn load_backup(
    path: String,
    pool: State<'_, SqlitePool>,
    storage: State<'_, AppStorage>,
    model_config: State<'_, ModelConfigState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let backup_path = PathBuf::from(path);

    if !backup_path.exists() {
        return Err(format!(
            "Backup file does not exist:\n{}",
            backup_path.display()
        ));
    }

    if backup_path
        .extension()
        .and_then(|x| x.to_str())
        .map(|x| x.eq_ignore_ascii_case("zip"))
        != Some(true)
    {
        return Err("Backup file must have a .zip extension.".to_string());
    }

    let staging = unique_staging_dir(&storage.dir, "backup-load");

    fs::create_dir_all(&staging).map_err(error_string)?;

    let result = async {
        /*
         * Extract + validate everything before touching
         * the live application.
         */
        let manifest = extract_backup(&backup_path, &staging)?;

        /*
         * Validate model configuration.
         */
        let model_text =
            fs::read_to_string(staging.join("config/model_config.json")).map_err(error_string)?;

        let restored_model: ModelConfig = serde_json::from_str(&model_text)
            .map_err(|e| format!("Invalid model configuration in backup: {}", e))?;

        /*
         * Make sure every manifest PDF exists.
         */
        for book in &manifest.books {
            let pdf = staging.join(&book.pdf_path);

            if !pdf.exists() {
                return Err(format!("Missing PDF for book '{}'", book.title));
            }
        }

        /*
         * Restore SQLite schema + data.
         *
         * This handles databases whose schemas differ.
         */
        restore_sqlite_database(pool.inner(), &staging.join("database/app.db"))
            .await
            .map_err(|e| format!("Failed to restore database: {}", e))?;

        /*
         * Rebuild the application's PDF directory.
         */
        let pdf_dir = storage.dir.join("pdfs");

        let new_pdf_dir = storage.dir.join("pdfs.restore");

        cleanup(&new_pdf_dir);

        fs::create_dir_all(&new_pdf_dir).map_err(error_string)?;

        for book in &manifest.books {
            let source = staging.join(&book.pdf_path);

            let extension = safe_pdf_extension(Path::new(&book.pdf_path));

            let destination = new_pdf_dir.join(format!("{}{}", book.id, extension));

            copy_file(&source, &destination)?;
        }

        /*
         * Replace the PDF directory.
         */
        let old_pdf_dir = storage.dir.join("pdfs.old");

        cleanup(&old_pdf_dir);

        if pdf_dir.exists() {
            fs::rename(&pdf_dir, &old_pdf_dir).map_err(error_string)?;
        }

        if let Err(error) = fs::rename(&new_pdf_dir, &pdf_dir) {
            /*
             * Try to put the previous directory back.
             */
            if old_pdf_dir.exists() && !pdf_dir.exists() {
                let _ = fs::rename(&old_pdf_dir, &pdf_dir);
            }

            return Err(error_string(error));
        }

        /*
         * Update books.file_path so it points to the
         * restored PDFs.
         */
        for book in &manifest.books {
            let extension = safe_pdf_extension(Path::new(&book.pdf_path));

            let new_path = pdf_dir.join(format!("{}{}", book.id, extension));

            sqlx::query(
                "UPDATE books
                 SET file_path = ?
                 WHERE id = ?",
            )
            .bind(new_path.to_string_lossy().to_string())
            .bind(&book.id)
            .execute(pool.inner())
            .await
            .map_err(error_string)?;
        }

        /*
         * Restore FAISS.
         */
        copy_file(
            &staging.join("faiss/faiss.index"),
            &storage.dir.join("faiss.index"),
        )?;

        /*
         * Restore model config.
         */
        copy_file(
            &staging.join("config/model_config.json"),
            &storage.dir.join("model_config.json"),
        )?;

        /*
         * Update in-memory model config.
         */
        {
            let mut config = model_config
                .config
                .lock()
                .map_err(|_| "Failed to lock model configuration".to_string())?;

            *config = restored_model;
        }

        cleanup(&old_pdf_dir);

        /*
         * Tell React/Vue/etc. that the whole library changed.
         */
        let _ = app.emit("backup-restored", ());

        Ok::<(), String>(())
    }
    .await;

    cleanup(&staging);

    result
}
