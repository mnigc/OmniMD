use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum InputType {
    File,
    Folder,
    Url,
}

// Aligned with the AnyDoc engine's supported formats. Detection is
// content-based; the extension is only a fallback for signature-less
// formats (CSV). Plain text/HTML and images are not supported (no OCR).
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "pdf",
    "doc", "docx", "docm",
    "ppt", "pps", "pot", "pptx", "pptm", "ppsx", "ppsm",
    "xls", "xlsx", "xlsm", "xlsb",
    "odt", "ods", "odp",
    "rtf", "epub", "csv",
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

/// Name of the per-document assets subdirectory, used both when writing files
/// and as the in-markdown reference prefix (`assets/xxx`).
pub const ASSET_DIR_NAME: &str = "assets";

/// Compute the final markdown output path and (optionally) the assets
/// directory for a conversion.
///
/// When `has_assets` is true, the output is laid out as a self-contained
/// bundle so the markdown and its images live together:
///   `{output_dir}/{stem}/{stem}.md` + `{output_dir}/{stem}/assets/`
/// When `has_assets` is false, no subdirectory is created and the markdown is
/// written directly to `{output_dir}/{stem}.md`, avoiding empty directories.
///
/// The markdown renderer already references assets as `assets/xxx`, which
/// resolves relative to the `.md` file under the bundled layout.
pub fn get_output_path_with_assets(
    input_path: &str,
    output_dir: &str,
    has_assets: bool,
) -> (PathBuf, Option<PathBuf>) {
    let stem = Path::new(input_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());

    let out = Path::new(output_dir);
    if has_assets {
        // Pick a unique bundle directory FIRST, so the markdown and its
        // `assets/` always live in the same fresh folder. Resolving only the
        // `.md` name (as before) let a second conversion of the same stem reuse
        // the first run's `assets/` directory and overwrite its images.
        let sub = resolve_unique_dir(out.join(&stem));
        let md = sub.join(format!("{}.md", stem));
        let asset_dir = sub.join(ASSET_DIR_NAME);
        (md, Some(asset_dir))
    } else {
        let md = resolve_unique_path(out.join(format!("{}.md", stem)));
        (md, None)
    }
}

/// Like [`resolve_unique_path`] but for a directory: returns the first
/// non-existing `{name}`, `{name}-1`, `{name}-2`, … path.
pub fn resolve_unique_dir(initial: PathBuf) -> PathBuf {
    if !initial.exists() {
        return initial;
    }
    let parent = initial
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let name = initial
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());

    let mut counter = 1;
    loop {
        let path = parent.join(format!("{}-{}", name, counter));
        if !path.exists() {
            return path;
        }
        counter += 1;
    }
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

/// List supported files in the top level of a directory only (no recursion).
pub fn list_files_flat(path: &str, extensions: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let p = Path::new(path);

    if p.is_file() {
        files.push(p.to_path_buf());
        return files;
    }

    if p.is_dir() {
        let entries = match std::fs::read_dir(p) {
            Ok(e) => e,
            Err(_) => return files,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if extensions.contains(&ext.to_lowercase().as_str()) {
                        files.push(path);
                    }
                }
            }
        }
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

pub fn get_supported_extensions_ref() -> &'static [&'static str] {
    SUPPORTED_EXTENSIONS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_unique_dir_avoids_existing_name() {
        let base = std::env::temp_dir().join(format!("omnimd_dir_test_{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        // Existing `doc` directory must push the result to `doc-1`.
        std::fs::create_dir_all(base.join("doc")).unwrap();
        let resolved = resolve_unique_dir(base.join("doc"));
        assert_eq!(
            resolved.file_name().and_then(|n| n.to_str()),
            Some("doc-1")
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn output_with_assets_keeps_md_and_assets_together() {
        let base = std::env::temp_dir().join(format!("omnimd_out_test_{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let (md, assets) =
            get_output_path_with_assets("D:/docs/report.pdf", base.to_str().unwrap(), true);
        let assets = assets.unwrap();
        assert_eq!(md.parent(), assets.parent());
        assert!(assets.ends_with(ASSET_DIR_NAME));
        std::fs::remove_dir_all(&base).ok();
    }
}
