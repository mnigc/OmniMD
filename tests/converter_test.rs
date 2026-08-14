use omnid_lib::converters::get_converter;
use omnid_lib::models::converter::Converter;

#[test]
fn test_converter_name() {
    let converter = get_converter();
    assert_eq!(converter.name(), "anydoc");
}

#[test]
fn test_supports_known_formats() {
    let converter = get_converter();
    assert!(converter.supports("pdf"));
    assert!(converter.supports("PDF"));
    assert!(converter.supports(".docx"));
    assert!(converter.supports("xlsx"));
    assert!(converter.supports("epub"));
    assert!(converter.supports("csv"));
    assert!(converter.supports("txt"));
    assert!(converter.supports("html"));
    assert!(converter.supports("rtf"));
    assert!(converter.supports("odt"));
}

#[test]
fn test_detect_pdf() {
    let converter = get_converter();
    let bytes = b"%PDF-1.4 stream xref trailer";
    assert_eq!(converter.detect_format(bytes), Some("pdf".to_string()));
}

#[test]
fn test_detect_docx() {
    let converter = get_converter();
    let mut bytes = vec![0u8; 50];
    bytes[0..4].copy_from_slice(b"PK\x03\x04");
    bytes[6..11].copy_from_slice(b"word/");
    assert_eq!(converter.detect_format(&bytes), Some("docx".to_string()));
}

#[test]
fn test_detect_epub() {
    let converter = get_converter();
    let mut bytes = vec![0u8; 50];
    bytes[0..4].copy_from_slice(b"PK\x03\x04");
    bytes[6..14].copy_from_slice(b"mimetype");
    assert_eq!(converter.detect_format(&bytes), Some("epub".to_string()));
}

#[test]
fn test_detect_unknown() {
    let converter = get_converter();
    let bytes = b"random binary data that doesn't match any format";
    assert_eq!(converter.detect_format(bytes), None);
}

#[test]
fn test_csv_conversion() {
    let converter = get_converter();
    let csv = "Name,Age,City\nAlice,30,NYC\nBob,25,LA\n";
    let result = converter.convert(csv.as_bytes());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.markdown.contains("Name"));
    assert!(r.markdown.contains("Alice"));
    assert!(r.errors.is_empty());
}

#[test]
fn test_txt_conversion() {
    let converter = get_converter();
    let text = "Hello, world!\nThis is a test.";
    let result = converter.convert(text.as_bytes());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.markdown.contains("Hello, world!"));
    assert!(r.errors.is_empty());
}

#[test]
fn test_html_conversion() {
    let converter = get_converter();
    let html = "<html><body><h1>Title</h1><p>Content</p></body></html>";
    let result = converter.convert(html.as_bytes());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.markdown.contains("Title") || r.markdown.contains("Content"));
}

#[test]
fn test_json_passthrough() {
    let converter = get_converter();
    let json = r#"{"key":"value","nested":{"a":1}}"#;
    let result = converter.convert(json.as_bytes());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.markdown.contains("key"));
    assert!(r.markdown.contains("value"));
}

#[test]
fn test_xml_passthrough() {
    let converter = get_converter();
    let xml = "<?xml version=\"1.0\"?><root><item>text</item></root>";
    let result = converter.convert(xml.as_bytes());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.markdown.contains("item"));
}
