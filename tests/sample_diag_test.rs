//! Diagnostic tests using the project's sample-data folder.
//!
//! These tests convert real multi-page sample PDFs through the full pipeline
//! (Auto OCR mode) and surface the tracing logs emitted by `convert_hybrid_pdf`
//! so we can see whether every page is rendered + OCR'd or only the first.

use omnid_lib::pipeline;
use omnid_lib::models::task::{ConversionTask, OutputMode};
use omnid_lib::converters::get_converter;
use omnid_lib::models::converter::Converter;

fn manifest_dir() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
}

fn sample(name: &str) -> String {
    format!("{}/样例数据/{}", manifest_dir(), name)
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_new("info,omnid_lib=debug,ocr=debug").unwrap_or_default();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_test_writer()
        .try_init();
}

/// Anydoc fast-path (no OCR): a digital text PDF should yield text from ALL
/// pages, not just the first. This exercises the fast path in convert_hybrid_pdf
/// and the anydoc converter directly.
#[tokio::test]
async fn anydoc_digital_pdf_emits_all_pages() {
    init_tracing();
    let src = sample("有赞文化手册_2022.pdf");
    if !std::path::Path::new(&src).exists() {
        eprintln!("skip: {} not found", src);
        return;
    }
    let bytes = std::fs::read(&src).unwrap();
    let converter = get_converter();
    let result = converter.convert(&bytes);
    match &result {
        Ok(r) => {
            eprintln!("anydoc: markdown_len={} errors={}", r.markdown.len(), r.errors.len());
            eprintln!("--- tail 300 ---\n{}", r.markdown.chars().rev().take(300).collect::<String>().chars().rev().collect::<String>());
        }
        Err(e) => eprintln!("anydoc ERR: {:?} {}", e.code, e.message),
    }
    assert!(result.is_ok(), "anydoc conversion should succeed");
    let r = result.unwrap();
    // A multi-page handbook should produce a substantial amount of text.
    // If only the first page is converted, this will be tiny.
    eprintln!("anydoc markdown length = {}", r.markdown.len());
    assert!(
        r.markdown.len() > 2000,
        "markdown suspiciously short ({}) — likely only the first page was converted",
        r.markdown.len()
    );
}

/// Full pipeline (Auto mode) on the digital PDF: should hit the fast path and
/// emit markdown covering all pages.
#[tokio::test]
async fn pipeline_digital_pdf_auto() {
    init_tracing();
    let src = sample("有赞文化手册_2022.pdf");
    if !std::path::Path::new(&src).exists() {
        eprintln!("skip: {} not found", src);
        return;
    }
    let out = format!("{}/target/test_out_yzan.md", manifest_dir());
    let task = ConversionTask::with_mode(&src, &out, OutputMode::Standard);

    let result = pipeline::convert_file(&task, None, None).await;
    match &result {
        Ok(r) => {
            eprintln!("pipeline: markdown_len={} errors={}", r.markdown.len(), r.errors.len());
            if let Some(s) = &r.stats {
                eprintln!(
                    "stats: ocr_pages={:?} ocr_chars={:?} words={} tables={}",
                    s.ocr_page_count, s.ocr_char_count, s.word_count, s.table_count
                );
            }
        }
        Err(e) => eprintln!("pipeline ERR: {:?} {}", e.code, e.message),
    }
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(
        r.markdown.len() > 2000,
        "pipeline markdown suspiciously short ({})",
        r.markdown.len()
    );
}

/// Full pipeline (Auto mode) on a scanned PDF: exercises the per-page OCR loop.
/// This is the path most likely to "only convert the first page".
#[tokio::test]
async fn pipeline_scanned_pdf_auto() {
    init_tracing();
    let src = sample("OCR 郭超-产品经理-4年经验-19957136704.pdf");
    if !std::path::Path::new(&src).exists() {
        eprintln!("skip: {} not found", src);
        return;
    }
    let out = format!("{}/target/test_out_guochao.md", manifest_dir());
    let task = ConversionTask::with_mode(&src, &out, OutputMode::Standard);

    let result = pipeline::convert_file(&task, None, None).await;
    match &result {
        Ok(r) => {
            eprintln!("pipeline(scanned): markdown_len={} errors={}", r.markdown.len(), r.errors.len());
            if let Some(s) = &r.stats {
                eprintln!(
                    "stats: ocr_pages={:?} ocr_chars={:?} words={} low_conf={:?}",
                    s.ocr_page_count, s.ocr_char_count, s.word_count, s.low_confidence_count
                );
            }
            for err in &r.errors {
                eprintln!("  page error: page={:?} {}", err.page, err.message);
            }
        }
        Err(e) => eprintln!("pipeline(scanned) ERR: {:?} {}", e.code, e.message),
    }
    assert!(result.is_ok());
    let r = result.unwrap();
    // A scanned multi-page PDF should produce OCR text from more than one page.
    if let Some(s) = &r.stats {
        eprintln!("ocr_pages = {:?}", s.ocr_page_count);
    }
    assert!(
        r.markdown.len() > 500,
        "scanned pipeline markdown too short ({})",
        r.markdown.len()
    );
}
