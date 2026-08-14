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
