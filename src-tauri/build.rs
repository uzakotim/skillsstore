use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // Configuration and launch of Tauri build
    let mut attributes = tauri_build::Attributes::new();

    if target_os == "windows" {
        // Statically link MSVC C runtime to eliminate missing VCRUNTIME140.dll on target Windows PCs
        let attrs = tauri_build::WindowsAttributes::new().static_vc_runtime(true);
        attributes = attributes.windows_attributes(attrs);
    }

    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}

