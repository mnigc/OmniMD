use std::fs;

fn fixture_dir() -> String {
    format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn test_get_output_path_basic() {
    let output = omnid_lib::file_utils::get_output_path(
        "/home/user/document.docx",
        "/home/user/output",
    );
    assert!(output.ends_with(".md"));
    assert!(output.contains("document"));
}

#[test]
fn test_get_output_path_conflict() {
    let dir = std::env::temp_dir();
    let existing = dir.join("test_conflict.md");
    fs::write(&existing, "exists").ok();

    let output = omnid_lib::file_utils::get_output_path(
        "test_conflict.txt",
        dir.to_str().unwrap(),
    );
    assert!(output.contains("-1"));

    fs::remove_file(&existing).ok();
}

#[test]
fn test_detect_input_type() {
    assert_eq!(
        omnid_lib::file_utils::detect_input_type("http://example.com/file.pdf"),
        omnid_lib::file_utils::InputType::Url
    );
    assert_eq!(
        omnid_lib::file_utils::detect_input_type("https://example.com/file.pdf"),
        omnid_lib::file_utils::InputType::Url
    );
}

#[test]
fn test_get_supported_extensions() {
    let exts = omnid_lib::file_utils::get_supported_extensions();
    assert!(exts.contains(&"pdf".to_string()));
    assert!(exts.contains(&"docx".to_string()));
    assert!(exts.contains(&"xlsx".to_string()));
    assert!(exts.contains(&"csv".to_string()));
    assert!(exts.contains(&"epub".to_string()));
    assert!(exts.contains(&"txt".to_string()));
}

#[test]
fn test_get_output_path_with_assets_bundles_when_has_assets() {
    use std::path::PathBuf;
    // Use a temp dir so nothing collides with existing files.
    let tmp = std::env::temp_dir().join(format!("omnid_assets_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let src = tmp.join("report.docx");
    let out_dir = tmp.join("out");
    std::fs::create_dir_all(&out_dir).unwrap();

    let (md, asset_dir): (PathBuf, Option<PathBuf>) =
        omnid_lib::file_utils::get_output_path_with_assets(
            src.to_str().unwrap(),
            out_dir.to_str().unwrap(),
            true,
        );
    // Bundled layout: <out>/<stem>/<stem>.md + <out>/<stem>/assets
    let md_str = md.to_string_lossy().replace('\\', "/");
    assert!(
        md_str.ends_with("report/report.md"),
        "expected bundled md, got {}",
        md_str
    );
    let asset = asset_dir.expect("asset dir should be Some when has_assets");
    let asset_str = asset.to_string_lossy().replace('\\', "/");
    assert!(
        asset_str.ends_with("report/assets"),
        "expected bundled asset dir, got {}",
        asset_str
    );
    assert!(md.starts_with(&out_dir));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn test_get_output_path_with_assets_flat_when_no_assets() {
    use std::path::PathBuf;
    let tmp = std::env::temp_dir().join(format!("omnid_flat_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let src = tmp.join("notes.txt");
    let out_dir = tmp.join("out");
    std::fs::create_dir_all(&out_dir).unwrap();

    let (md, asset_dir): (PathBuf, Option<PathBuf>) =
        omnid_lib::file_utils::get_output_path_with_assets(
            src.to_str().unwrap(),
            out_dir.to_str().unwrap(),
            false,
        );
    // Flat layout: <out>/<stem>.md, no asset dir, no empty subdir.
    let md_str = md.to_string_lossy().replace('\\', "/");
    assert!(md_str.ends_with("notes.md"), "expected flat md, got {}", md_str);
    assert_eq!(asset_dir, None);
    // The parent should be exactly out_dir (no stem subdir).
    assert_eq!(md.parent(), Some(out_dir.as_path()));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn test_list_files_recursive() {
    let dir = std::env::temp_dir();
    let test_dir = dir.join("omnid_test_list");
    fs::create_dir_all(&test_dir).ok();

    fs::write(test_dir.join("a.csv"), "test").ok();
    fs::write(test_dir.join("b.txt"), "test").ok();
    fs::write(test_dir.join("c.pdf"), "%PDF-1.4").ok();
    fs::write(test_dir.join("d.exe"), "test").ok();

    let files = omnid_lib::file_utils::list_files_recursive(
        test_dir.to_str().unwrap(),
        &["csv", "txt", "pdf"],
    );

    assert_eq!(files.len(), 3);
    let names: Vec<String> = files.iter().map(|f| f.file_name().unwrap().to_str().unwrap().to_string()).collect();
    assert!(names.contains(&"a.csv".to_string()));
    assert!(names.contains(&"b.txt".to_string()));
    assert!(names.contains(&"c.pdf".to_string()));
    assert!(!names.contains(&"d.exe".to_string()));

    fs::remove_dir_all(&test_dir).ok();
}
