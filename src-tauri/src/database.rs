use sqlx::{Acquire, SqlitePool};

// ─── Database Backup / Restore ────────────────────────────────────────────────

#[tauri::command]
pub async fn save_database(path: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
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
pub async fn load_database(path: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
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
