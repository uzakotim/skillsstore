use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::Emitter;

// ─── Ollama API Types ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct OllamaListResponse {
    pub models: Vec<OllamaModelInfo>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size: Option<u64>,
    pub digest: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OllamaTag {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "pulls")]
    pub pulls: Option<u64>,
}

#[derive(Serialize)]
pub struct OllamaPullRequest {
    pub name: String,
    pub stream: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ModelConfig {
    pub llm_model: String,
    pub embed_model: String,
}

#[derive(Deserialize)]
pub struct OllamaPullProgress {
    pub status: String,
    pub completed: Option<u64>,
    pub total: Option<u64>,
}

#[derive(Serialize, Clone)]
pub struct PullProgress {
    pub status: String,
    pub completed: Option<u64>,
    pub total: Option<u64>,
    pub percent: Option<f64>,
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

pub struct OllamaProcess {
    pub child: Arc<Mutex<Option<Child>>>,
}

impl OllamaProcess {
    pub fn new() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
        }
    }
}

// ─── Process Utilities ────────────────────────────────────────────────────────

pub fn kill_existing_ollama() {
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

pub fn find_ollama() -> Result<String, String> {
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

pub fn parse_prompt_processing_progress(line: &str) -> Option<(u64, u64)> {
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

// ─── Ollama HTTP API Helpers ──────────────────────────────────────────────────

#[derive(Serialize)]
pub struct OllamaGenerateRequest {
    pub model: String,
    pub prompt: String,
    pub stream: bool,
}

#[derive(Deserialize)]
pub struct OllamaGenerateResponse {
    pub response: String,
}

pub async fn call_ollama(model: &str, prompt: String) -> Result<String, String> {
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

pub async fn embed_with_model(text: &str, model: &str) -> Result<Vec<f32>, String> {
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

// ─── Ollama Tauri Commands ────────────────────────────────────────────────────

#[tauri::command]
pub fn start_ollama(
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
pub async fn list_local_models() -> Result<Vec<OllamaModelInfo>, String> {
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
pub async fn delete_ollama_model(name: String) -> Result<(), String> {
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
pub async fn pull_model(name: String, app: tauri::AppHandle) -> Result<(), String> {
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
pub async fn get_model_config(
    state: tauri::State<'_, ModelConfigState>,
) -> Result<ModelConfig, String> {
    let config = state.config.lock().unwrap();
    Ok(ModelConfig {
        llm_model: config.llm_model.clone(),
        embed_model: config.embed_model.clone(),
    })
}

#[tauri::command]
pub async fn set_model_config(
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
