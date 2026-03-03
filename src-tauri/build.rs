use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    if target_os == "macos" {
        // 1. Try to find libomp via brew
        let brew_prefix = Command::new("brew")
            .args(&["--prefix", "libomp"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| {
                if target_arch == "aarch64" {
                    "/opt/homebrew/opt/libomp".to_string()
                } else {
                    "/usr/local/opt/libomp".to_string()
                }
            });

        let omp_lib_path = PathBuf::from(&brew_prefix).join("lib");
        
        if omp_lib_path.exists() {
            println!("cargo:rustc-link-search=native={}", omp_lib_path.display());
            println!("cargo:rustc-link-arg=-lomp");
            
            // 2. Handle the 'gomp' shim for faiss-sys
            // We use the local native_libs directory relative to the crate
            let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
            let shim_dir = PathBuf::from(manifest_dir).join("native_libs");
            
            if shim_dir.exists() {
                println!("cargo:rustc-link-search=native={}", shim_dir.display());
            }
            
            // Link libomp explicitly for this crate
            println!("cargo:rustc-link-lib=omp");
        }
    }

    tauri_build::build();
}
