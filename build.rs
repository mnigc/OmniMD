use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let ppocr_dir = PathBuf::from(&manifest_dir).join("ocr_resources/ppocrv6");

    if ppocr_dir.exists() {
        eprintln!(
            "cargo:warning=PP-OCRv6 resources found at {}",
            ppocr_dir.display()
        );
    } else {
        let _ = fs::create_dir_all(&ppocr_dir);
        eprintln!(
            "cargo:warning=PP-OCRv6 resources directory created at {}. Place model files there.",
            ppocr_dir.display()
        );
    }

    tauri_build::build();
}