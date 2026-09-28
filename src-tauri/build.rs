use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();

    // 🍏 Конфигурация для macOS
    if target_os == "macos" {
        // 1. Try to find libomp via brew, but fall back when brew is unavailable
        let fallback_prefix = if target_arch == "aarch64" {
            "/opt/homebrew/opt/libomp".to_string()
        } else {
            "/usr/local/opt/libomp".to_string()
        };
        let brew_prefix = match Command::new("brew").args(["--prefix", "libomp"]).output() {
            Ok(output) if output.status.success() => {
                let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if prefix.is_empty() {
                    fallback_prefix
                } else {
                    prefix
                }
            }
            _ => fallback_prefix,
        };

        let omp_lib_path = PathBuf::from(&brew_prefix).join("lib");

        if omp_lib_path.exists() {
            println!("cargo:rustc-link-search=native={}", omp_lib_path.display());
            println!("cargo:rustc-link-lib=omp");
        }

        // 2. Always add the local gomp shim directory on macOS when present.
        let shim_dir = PathBuf::from(&manifest_dir).join("native_libs");
        if shim_dir.exists() {
            println!("cargo:rustc-link-search=native={}", shim_dir.display());
        }
    }

    // Конфигурация для Windows
    if target_os == "windows" {
        // Если для сборки под Windows вам тоже нужны локальные библиотеки (.lib / .dll),
        // этот блок автоматически подключит папку "native_libs/windows" (если она существует)
        let win_lib_dir = PathBuf::from(&manifest_dir)
            .join("native_libs")
            .join("windows");
        if win_lib_dir.exists() {
            println!("cargo:rustc-link-search=native={}", win_lib_dir.display());
            // Здесь при необходимости можно раскомментировать линковку конкретной библиотеки:
            // println!("cargo:rustc-link-lib=static=имя_библиотеки");
        }
    }

    // Настройка и запуск сборщика Tauri
    let mut attributes = tauri_build::Attributes::new();

    if target_os == "windows" {
        // Заменяем устаревший STATIC_VCRUNTIME на актуальный метод
        let attrs = tauri_build::WindowsAttributes::new().static_vc_runtime(true);
        attributes = attributes.windows_attributes(attrs);
    }

    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
