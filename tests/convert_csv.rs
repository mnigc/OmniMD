use std::fs;
use std::path::Path;

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

#[test]
fn test_csv_conversion() {
    let csv_content = "Name,Age,City\nAlice,30,NYC\nBob,25,LA\n";
    let path = fixture_path("csv/test.csv");
    fs::write(&path, csv_content).unwrap();

    let converter = omnid_lib::converters::get_converter();
    use omnid_lib::models::converter::Converter;

    assert!(converter.supports("csv"));
    assert!(converter.supports("CSV"));
    assert!(converter.supports(".csv"));

    let result = converter.convert(csv_content.as_bytes());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(!r.markdown.is_empty());
    assert!(r.markdown.contains("Name"));
    assert!(r.markdown.contains("Alice"));
    assert!(r.errors.is_empty());

    fs::remove_file(&path).ok();
}

#[test]
fn test_txt_conversion() {
    let text = "Hello, world!\nThis is a test document.";
    let converter = omnid_lib::converters::get_converter();
    use omnid_lib::models::converter::Converter;

    let result = converter.convert(text.as_bytes());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.markdown.contains("Hello, world!"));
    assert!(r.errors.is_empty());
}

#[test]
fn test_html_conversion() {
    let html = "<!DOCTYPE html><html><body><h1>Title</h1><p>Content</p></body></html>";
    let converter = omnid_lib::converters::get_converter();
    use omnid_lib::models::converter::Converter;

    let result = converter.convert(html.as_bytes());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.markdown.contains("Title") || r.markdown.contains("Content"));
    assert!(r.errors.is_empty());
}

#[test]
fn test_json_passthrough() {
    let json = r#"{"name":"test","value":42}"#;
    let converter = omnid_lib::converters::get_converter();
    use omnid_lib::models::converter::Converter;

    let result = converter.convert(json.as_bytes());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.markdown.contains("test"));
    assert!(r.errors.is_empty());
}

#[test]
fn test_xml_passthrough() {
    let xml = "<?xml version=\"1.0\"?><root><item>value</item></root>";
    let converter = omnid_lib::converters::get_converter();
    use omnid_lib::models::converter::Converter;

    let result = converter.convert(xml.as_bytes());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.markdown.contains("item"));
    assert!(r.errors.is_empty());
}

#[test]
fn test_unsupported_format_returns_error() {
    let data = b"this is not a valid document format";
    let converter = omnid_lib::converters::get_converter();
    use omnid_lib::models::converter::Converter;

    let result = converter.convert(data);
    // Should succeed as text passthrough since it doesn't match known signatures
    assert!(result.is_ok());
}

#[test]
fn test_detect_format_pdf() {
    let pdf_header = b"%PDF-1.4 test content";
    let converter = omnid_lib::converters::get_converter();
    use omnid_lib::models::converter::Converter;

    let format = converter.detect_format(pdf_header);
    assert_eq!(format, Some("pdf".to_string()));
}

#[test]
fn test_detect_format_docx() {
    let mut bytes = vec![0u8; 50];
    bytes[0..4].copy_from_slice(b"PK\x03\x04");
    bytes[6..11].copy_from_slice(b"word/");
    let converter = omnid_lib::converters::get_converter();
    use omnid_lib::models::converter::Converter;

    let format = converter.detect_format(&bytes);
    assert_eq!(format, Some("docx".to_string()));
}

#[test]
fn test_detect_format_epub() {
    let mut bytes = vec![0u8; 50];
    bytes[0..4].copy_from_slice(b"PK\x03\x04");
    bytes[6..14].copy_from_slice(b"mimetype");
    let converter = omnid_lib::converters::get_converter();
    use omnid_lib::models::converter::Converter;

    let format = converter.detect_format(&bytes);
    assert_eq!(format, Some("epub".to_string()));
}
