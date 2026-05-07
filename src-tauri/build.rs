use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    if target_os == "macos" {
        // 1. Try to find libomp via brew, but fall back when brew is unavailable
        // or returns a non-zero exit status.
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
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let shim_dir = PathBuf::from(manifest_dir).join("native_libs");
        if shim_dir.exists() {
            println!("cargo:rustc-link-search=native={}", shim_dir.display());
        }
    }

    tauri_build::build();
}
