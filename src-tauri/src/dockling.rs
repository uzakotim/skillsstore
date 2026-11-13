use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::Manager;

// ─── DocklingProcess State ─────────────────────────────────────────────────────

pub struct DocklingProcess {
    pub child: Arc<Mutex<Option<Child>>>,
}

impl DocklingProcess {
    pub fn new() -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
        }
    }
}

// ─── Start / Stop ─────────────────────────────────────────────────────────────

pub fn start_dockling(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let backend_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("dockling_backend");
        let script = backend_dir.join("main.py");
        if !script.is_file() {
            eprintln!("[Dockling] Backend script not found: {}", script.display());
            return;
        }

        let venv_python = backend_dir.join(".venv").join("bin").join("python");
        let python = if venv_python.is_file() {
            venv_python
        } else {
            std::path::PathBuf::from("python3")
        };

        println!("[Dockling] Starting {}", script.display());
        let child = match Command::new(&python)
            .arg(&script)
            .current_dir(&backend_dir)
            .stdin(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                eprintln!("[Dockling] Failed to start {}: {}", python.display(), e);
                return;
            }
        };

        app.state::<DocklingProcess>().child.lock().unwrap().replace(child);
        println!("[Dockling] Backend process started.");
    });
}

pub fn stop_dockling(state: &DocklingProcess) {
    if let Ok(mut guard) = state.child.lock() {
        if let Some(child) = guard.take() {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            println!("[Dockling] Backend stopped.");
        }
    }
}

// ─── Tauri Command ────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct ConvertRequest {
    path: String,
}

#[derive(serde::Deserialize)]
struct ConvertResponse {
    markdown: String,
}

#[tauri::command]
pub async fn convert_pdf_to_markdown(path: String) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let mut last_error = None;
    let mut response = None;
    for attempt in 0..60 {
        match client
            .post("http://127.0.0.1:48484/convert")
            .json(&ConvertRequest { path: path.clone() })
            .send()
            .await
        {
            Ok(result) => {
                response = Some(result);
                break;
            }
            Err(error) => {
                last_error = Some(error);
                if attempt < 59 {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
    }
    let response = response.ok_or_else(|| {
        format!(
            "Failed to reach Docling backend at 127.0.0.1:48484 after 60 seconds: {}. Run dockling_backend/setup.sh to install its Python dependencies.",
            last_error.map(|e| e.to_string()).unwrap_or_else(|| "unknown connection error".into())
        )
    })?;

    let status = response.status();

    if status.is_success() {
        let body: ConvertResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse dockling response: {}", e))?;
        Ok(body.markdown)
    } else {
        let text = response
            .text()
            .await
            .unwrap_or_else(|_| format!("HTTP {}", status));
        Err(format!("Dockling backend error ({}): {}", status, text))
    }
}
