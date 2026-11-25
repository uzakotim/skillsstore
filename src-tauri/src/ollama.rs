use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
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
                        llm_model: String::new(),
                        embed_model: String::new(),
                    }
                }),
                Err(e) => {
                    eprintln!("Failed to read model_config.json at {:?}: {}", path, e);
                    ModelConfig {
                        llm_model: String::new(),
                        embed_model: String::new(),
                    }
                }
            }
        } else {
            let default_cfg = ModelConfig {
                llm_model: String::new(),
                embed_model: String::new(),
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
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "Ollama.exe", "/T"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "llama-server.exe", "/T"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("/usr/bin/pkill")
            .args(["-i", "-x", "ollama"])
            .output();
        let _ = Command::new("/usr/bin/pkill")
            .args(["-x", "Ollama"])
            .output();
        let _ = Command::new("/usr/bin/pkill")
            .args(["-i", "-x", "llama-server"])
            .output();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("/usr/bin/pkill")
            .args(["-i", "-x", "ollama"])
            .output();
        let _ = Command::new("/usr/bin/pkill")
            .args(["-i", "-x", "llama-server"])
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

pub fn parse_analysis_progress(line: &str) -> Option<AnalysisProgress> {
    static RE_ANSI: OnceLock<Regex> = OnceLock::new();
    let re_ansi = RE_ANSI.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap());
    let clean = re_ansi.replace_all(line, "");
    let clean_lower = clean.to_lowercase();

    // Check if line indicates prompt processing or prompt eval
    if !clean_lower.contains("prompt processing")
        && !clean_lower.contains("prompt eval")
        && !clean_lower.contains("processing prompt")
    {
        return None;
    }

    // 1. llama-server format: "prompt processing, n_tokens = <num>, progress = <float>"
    static RE_LLAMA: OnceLock<Regex> = OnceLock::new();
    let re_llama = RE_LLAMA.get_or_init(|| {
        Regex::new(r"(?i)prompt processing[^\n]*?n_tokens\s*=\s*(\d+)[^\n]*?progress\s*=\s*([0-9.]+)")
            .unwrap()
    });
    if let Some(caps) = re_llama.captures(&clean) {
        if let (Ok(n_tokens), Ok(prog_val)) = (caps[1].parse::<u64>(), caps[2].parse::<f64>()) {
            let (percent, total) = if prog_val > 1.0 {
                let pct = prog_val.clamp(0.0, 100.0);
                let tot = if pct > 0.0 {
                    ((n_tokens as f64 / (pct / 100.0)).round() as u64).max(n_tokens)
                } else {
                    n_tokens
                };
                (pct, tot)
            } else if prog_val > 0.0 {
                let pct = (prog_val * 100.0).clamp(0.0, 100.0);
                let tot = ((n_tokens as f64 / prog_val).round() as u64).max(n_tokens);
                (pct, tot)
            } else {
                (0.0, n_tokens)
            };

            return Some(AnalysisProgress {
                status: "Processing book context".to_string(),
                processed: Some(n_tokens),
                total: Some(total),
                percent: Some(percent),
            });
        }
    }

    // 2. JSON log format: {"msg":"Prompt processing progress","processed":2048,"total":9664}
    let trimmed = clean.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let msg = val.get("msg").and_then(|m| m.as_str()).unwrap_or("");
            if msg.to_lowercase().contains("prompt processing")
                || msg.to_lowercase().contains("prompt eval")
            {
                let proc = val.get("processed").and_then(|p| p.as_u64());
                let tot = val.get("total").and_then(|t| t.as_u64());
                if let (Some(p), Some(t)) = (proc, tot) {
                    if t > 0 {
                        let pct = (p as f64 / t as f64) * 100.0;
                        return Some(AnalysisProgress {
                            status: "Processing book context".to_string(),
                            processed: Some(p),
                            total: Some(t),
                            percent: Some(pct),
                        });
                    }
                }
            }
        }
    }

    // 3. MLX runner / slog key-value format: processed=... total=...
    static RE_PROC: OnceLock<Regex> = OnceLock::new();
    static RE_TOT: OnceLock<Regex> = OnceLock::new();
    let re_proc = RE_PROC.get_or_init(|| Regex::new(r#"(?i)processed["\s:=]+(\d+)"#).unwrap());
    let re_tot = RE_TOT.get_or_init(|| Regex::new(r#"(?i)total["\s:=]+(\d+)"#).unwrap());
    if let (Some(cap_p), Some(cap_t)) = (re_proc.captures(&clean), re_tot.captures(&clean)) {
        if let (Ok(p), Ok(t)) = (cap_p[1].parse::<u64>(), cap_t[1].parse::<u64>()) {
            if t > 0 {
                let pct = (p as f64 / t as f64) * 100.0;
                return Some(AnalysisProgress {
                    status: "Processing book context".to_string(),
                    processed: Some(p),
                    total: Some(t),
                    percent: Some(pct),
                });
            }
        }
    }

    // 4. Ratio format like "1234 / 5678" or "1234/5678 tokens"
    static RE_RATIO: OnceLock<Regex> = OnceLock::new();
    let re_ratio = RE_RATIO.get_or_init(|| Regex::new(r"(\d+)\s*/\s*(\d+)").unwrap());
    if let Some(caps) = re_ratio.captures(&clean) {
        if let (Ok(p), Ok(t)) = (caps[1].parse::<u64>(), caps[2].parse::<u64>()) {
            if t > 0 {
                let pct = (p as f64 / t as f64) * 100.0;
                return Some(AnalysisProgress {
                    status: "Processing book context".to_string(),
                    processed: Some(p),
                    total: Some(t),
                    percent: Some(pct),
                });
            }
        }
    }

    // 5. Percentage format like "45%"
    static RE_PCT: OnceLock<Regex> = OnceLock::new();
    let re_pct = RE_PCT.get_or_init(|| Regex::new(r"(\d+(?:\.\d+)?)\s*%").unwrap());
    if let Some(caps) = re_pct.captures(&clean) {
        if let Ok(pct) = caps[1].parse::<f64>() {
            return Some(AnalysisProgress {
                status: "Processing book context".to_string(),
                processed: None,
                total: None,
                percent: Some(pct),
            });
        }
    }

    None
}

pub fn parse_prompt_processing_progress(line: &str) -> Option<(u64, u64)> {
    let progress = parse_analysis_progress(line)?;
    match (progress.processed, progress.total) {
        (Some(p), Some(t)) if t > 0 => Some((p, t)),
        _ => None,
    }
}

// ─── Ollama HTTP API Helpers ──────────────────────────────────────────────────

#[derive(Serialize)]
pub struct OllamaOptions {
    pub temperature: f32,
}

#[derive(Serialize)]
pub struct OllamaGenerateRequest {
    pub model: String,
    pub prompt: String,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<OllamaOptions>,
}

#[derive(Deserialize)]
pub struct OllamaGenerateResponse {
    pub response: String,
}

pub async fn call_ollama(model: &str, prompt: String) -> Result<String, String> {
    if model.trim().is_empty() {
        return Err("No LLM model selected. Please select a model in Model Manager.".to_string());
    }
    let client = reqwest::Client::new();
    let res = client
        .post("http://localhost:11434/api/generate")
        .json(&OllamaGenerateRequest {
            model: model.to_string(),
            prompt,
            stream: false,
            options: Some(OllamaOptions { temperature: 0.1 }),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let body: OllamaGenerateResponse = res.json().await.map_err(|e| e.to_string())?;
    Ok(body.response)
}

pub async fn embed_with_model(text: &str, model: &str) -> Result<Vec<f32>, String> {
    if model.trim().is_empty() {
        return Err("No embedding model selected. Please select a model in Model Manager.".to_string());
    }
    #[derive(serde::Serialize)]
    struct EmbedReq<'a> {
        model: &'a str,
        input: &'a str,
    }
    #[derive(serde::Deserialize)]
    struct EmbedResp {
        embeddings: Vec<Vec<f32>>,
    }

    let client = reqwest::Client::new();
    let res = client
        .post("http://localhost:11434/api/embed")
        .json(&EmbedReq {
            model,
            input: text,
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let body: EmbedResp = res.json().await.map_err(|e| e.to_string())?;
    body.embeddings
        .into_iter()
        .next()
        .ok_or_else(|| "Ollama returned empty embeddings".to_string())
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
    std::thread::sleep(std::time::Duration::from_millis(200));
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

    let handle_output_line = {
        let app_handle = app.clone();
        move |line: &str| {
            println!("[Ollama] {}", line);

            if let Some(payload) = parse_analysis_progress(line) {
                let _ = app_handle.emit("analysis-progress", payload);
            }
        }
    };

    // Ollama normally writes server logs to stderr.
    if let Some(stderr) = child.stderr.take() {
        let on_line = handle_output_line.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);

            for line in reader.lines() {
                let Ok(line) = line else {
                    break;
                };
                on_line(&line);
            }
        });
    }

    // Also listen to stdout in case a future Ollama version uses it.
    if let Some(stdout) = child.stdout.take() {
        let on_line = handle_output_line;
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);

            for line in reader.lines() {
                let Ok(line) = line else {
                    break;
                };
                on_line(&line);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mlx_log() {
        let line = r#"time=2026-11-23T16:51:07.329+08:00 level=INFO source=pipeline.go:252 msg="Prompt processing progress" processed=2048 total=9664"#;
        let p = parse_analysis_progress(line).expect("should parse MLX log");
        assert_eq!(p.processed, Some(2048));
        assert_eq!(p.total, Some(9664));
        assert!((p.percent.unwrap() - 21.19).abs() < 0.1);

        let (proc, tot) = parse_prompt_processing_progress(line).unwrap();
        assert_eq!(proc, 2048);
        assert_eq!(tot, 9664);
    }

    #[test]
    fn test_parse_mlx_json_log() {
        let line = r#"{"time":"2026-11-23T16:51:07.329Z","level":"INFO","source":"pipeline.go:252","msg":"Prompt processing progress","processed":2048,"total":9664}"#;
        let p = parse_analysis_progress(line).expect("should parse JSON log");
        assert_eq!(p.processed, Some(2048));
        assert_eq!(p.total, Some(9664));
        assert!((p.percent.unwrap() - 21.19).abs() < 0.1);
    }

    #[test]
    fn test_parse_llama_server_log() {
        let line = "slot update_slots: id  0 | task 0 | prompt processing, n_tokens =   1536, progress = 0.38, t =   0.45 s / 3413.33 tokens per second";
        let p = parse_analysis_progress(line).expect("should parse llama-server log");
        assert_eq!(p.processed, Some(1536));
        assert_eq!(p.total, Some(4042));
        assert!((p.percent.unwrap() - 38.0).abs() < 0.01);

        let (proc, tot) = parse_prompt_processing_progress(line).unwrap();
        assert_eq!(proc, 1536);
        assert_eq!(tot, 4042);
    }

    #[test]
    fn test_parse_llama_server_complete() {
        let line = "slot %12s: id 0 | task 0 | prompt processing, n_tokens = 500, progress = 1.00, t = 0.10 s";
        let p = parse_analysis_progress(line).expect("should parse 100% progress");
        assert_eq!(p.processed, Some(500));
        assert_eq!(p.total, Some(500));
        assert_eq!(p.percent, Some(100.0));
    }

    #[test]
    fn test_parse_unrelated_log() {
        let line = "time=2026-11-23T16:51:07.329+08:00 level=INFO source=server.go:100 msg=\"listening on 127.0.0.1:11434\"";
        assert!(parse_analysis_progress(line).is_none());
        assert!(parse_prompt_processing_progress(line).is_none());
    }

    #[tokio::test]
    #[ignore = "requires local Ollama instance with gemma4:e4b-mlx"]
    async fn test_call_ollama_gemma() {
        let prompt = "Respond with 'Hello, world!' and nothing else.".to_string();
        let res = call_ollama("gemma4:e4b-mlx", prompt).await;
        println!("\n--- Ollama Response ---\n{:?}\n-----------------------", res);
        assert!(res.is_ok());
    }
}

