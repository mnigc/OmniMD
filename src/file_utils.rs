use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum InputType {
    File,
    Folder,
    Url,
}

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "doc", "pptx", "ppt", "xlsx", "xls", "epub", "csv", "txt", "html", "htm",
    "odt", "ods", "odp", "rtf",
];

pub fn detect_input_type(path: &str) -> InputType {
    if path.starts_with("http://") || path.starts_with("https://") {
        return InputType::Url;
    }

    let p = Path::new(path);
    if p.is_dir() {
        InputType::Folder
    } else {
        InputType::File
    }
}

pub fn get_output_path(input_path: &str, output_dir: &str) -> String {
    let input = Path::new(input_path);
    let file_stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());

    let output = Path::new(output_dir).join(format!("{}.md", file_stem));
    resolve_unique_path(output)
        .to_string_lossy()
        .to_string()
}

pub fn resolve_unique_path(initial: PathBuf) -> PathBuf {
    if !initial.exists() {
        return initial;
    }

    let parent = initial
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let stem = initial
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    let extension = initial
        .extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut counter = 1;
    loop {
        let new_name = format!("{}-{}", stem, counter);
        let path = parent.join(if extension.is_empty() {
            new_name
        } else {
            format!("{}.{}", new_name, extension)
        });
        if !path.exists() {
            return path;
        }
        counter += 1;
    }
}

pub fn list_files_recursive(path: &str, extensions: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let p = Path::new(path);

    if p.is_file() {
        files.push(p.to_path_buf());
        return files;
    }

    if p.is_dir() {
        collect_files(p, extensions, &mut files);
    }

    files
}

fn collect_files(dir: &Path, extensions: &[&str], result: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, extensions, result);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if extensions.contains(&ext.to_lowercase().as_str()) {
                result.push(path);
            }
        }
    }
}

pub fn get_supported_extensions() -> Vec<String> {
    SUPPORTED_EXTENSIONS.iter().map(|s| s.to_string()).collect()
}
