use std::env;
use std::path::Path;

fn main() {
    // Tell Cargo to rerun this build script if the webui directory changes
    let webui_path =
        env::var("MOXNOTIFY_WEBUI_DIR").unwrap_or_else(|_| "../webui/dist".to_string());

    let webui_path = Path::new(&webui_path);
    if webui_path.exists() {
        println!("cargo:rerun-if-changed={}", webui_path.display());
        // Walk the directory to watch all files
        if let Ok(entries) = std::fs::read_dir(webui_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    println!("cargo:rerun-if-changed={}", path.display());
                }
            }
        }
    }
}
