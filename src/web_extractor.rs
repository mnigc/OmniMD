use std::time::Duration;

use scraper::{ElementRef, Html, Node, Selector};

pub struct ExtractedContent {
    pub title: String,
    pub markdown: String,
    pub source_url: String,
}

pub async fn fetch_html(url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch URL: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Server returned error status: {}", response.status()));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.contains("text/html") && !content_type.contains("application/xhtml") {
        return Err(format!("Unsupported content type: {}", content_type));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    if bytes.len() > 10 * 1024 * 1024 {
        return Err("Response too large (max 10MB)".to_string());
    }

    let html = String::from_utf8_lossy(&bytes).to_string();
    Ok(html)
}

pub fn extract_content(html: &str, source_url: &str) -> Result<ExtractedContent, String> {
    let document = Html::parse_document(html);
    let title = extract_title(&document);
    let markdown = extract_main_content_to_markdown(&document);
    Ok(ExtractedContent {
        title: title.clone(),
        markdown,
        source_url: source_url.to_string(),
    })
}

fn extract_title(document: &Html) -> String {
    if let Ok(selector) = Selector::parse("title") {
        if let Some(element) = document.select(&selector).next() {
            let text = element.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                return text;
            }
        }
    }
    if let Ok(selector) = Selector::parse("h1") {
        if let Some(element) = document.select(&selector).next() {
            let text = element.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                return text;
            }
        }
    }
    String::new()
}

fn extract_main_content_to_markdown(document: &Html) -> String {
    let content_selectors = [
        "article",
        "[role=main]",
        "main",
        ".post-content",
        ".entry-content",
        ".article-content",
        ".content",
        "#content",
        "#main-content",
        ".main-content",
    ];

    for selector_str in &content_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(element) = document.select(&selector).next() {
                let result = element_to_markdown(&element);
                if !result.trim().is_empty() {
                    return result;
                }
            }
        }
    }

    if let Ok(selector) = Selector::parse("body") {
        if let Some(element) = document.select(&selector).next() {
            return element_to_markdown(&element);
        }
    }

    String::new()
}

fn is_noise_element(el: &ElementRef) -> bool {
    let tag = el.value().name.local.as_ref();
    let noise_tags = [
        "nav", "footer", "aside", "header", "script", "style", "noscript",
    ];
    if noise_tags.contains(&tag) {
        return true;
    }

    let role = el.value().attr("role").unwrap_or("");
    if role == "navigation" || role == "complementary" || role == "contentinfo" {
        return true;
    }

    let class_str = el.value().attr("class").unwrap_or("");
    let noise_classes = [
        "sidebar", "advertisement", "ad", "cookie-banner", "cookie-consent",
        "comment", "comments", "social-share", "share-buttons", "related-posts",
        "recommendations", "newsletter", "subscribe", "menu", "navbar",
        "breadcrumb", "breadcrumbs", "widget", "widget-area",
    ];
    for cls in &noise_classes {
        if class_str.split_whitespace().any(|c| c == *cls || c.starts_with(cls) || c.ends_with(cls)) {
            return true;
        }
    }

    let id = el.value().attr("id").unwrap_or("");
    let noise_ids = ["sidebar", "comments", "comment", "footer", "header", "nav", "menu"];
    if noise_ids.contains(&id) {
        return true;
    }

    false
}

fn element_to_markdown(el: &ElementRef) -> String {
    if is_noise_element(el) {
        return String::new();
    }

    let tag = el.value().name.local.as_ref();
    match tag {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = tag[1..].parse::<usize>().unwrap_or(1);
            let content = collect_text(el);
            if content.trim().is_empty() {
                return String::new();
            }
            let prefix = "#".repeat(level);
            format!("{} {}\n\n", prefix, content.trim())
        }
        "p" => {
            let content = inline_to_markdown(el);
            if content.trim().is_empty() {
                return String::new();
            }
            format!("{}\n\n", content.trim())
        }
        "br" => "  \n".to_string(),
        "hr" => "---\n\n".to_string(),
        "ul" | "ol" => {
            let ordered = tag == "ol";
            let mut result = String::new();
            let mut index = 0;
            for child in el.children() {
                if let Some(child_el) = ElementRef::wrap(child) {
                    if child_el.value().name.local.as_ref() == "li" {
                        let content = inline_to_markdown(&child_el);
                        if content.trim().is_empty() {
                            continue;
                        }
                        index += 1;
                        let marker = if ordered {
                            format!("{}.", index)
                        } else {
                            "-".to_string()
                        };
                        result.push_str(&format!("{} {}\n", marker, content.trim()));
                    }
                }
            }
            if !result.is_empty() {
                result.push('\n');
            }
            result
        }
        "blockquote" => {
            let content = render_children(el);
            if content.trim().is_empty() {
                return String::new();
            }
            let mut result = String::new();
            for line in content.lines() {
                if line.trim().is_empty() {
                    result.push_str(">\n");
                } else {
                    result.push_str(&format!("> {}\n", line));
                }
            }
            result.push('\n');
            result
        }
        "pre" => {
            let code_content = if let Some(code) = el.children().find_map(|child| {
                ElementRef::wrap(child).filter(|ce| ce.value().name.local.as_ref() == "code")
            }) {
                collect_text(&code)
            } else {
                collect_text(el)
            };
            if code_content.trim().is_empty() {
                return String::new();
            }
            let lang = if let Some(code) = el.children().find_map(|child| {
                ElementRef::wrap(child).filter(|ce| ce.value().name.local.as_ref() == "code")
            }) {
                code.value()
                    .attr("class")
                    .unwrap_or("")
                    .split(' ')
                    .find(|c| c.starts_with("language-") || c.starts_with("lang-"))
                    .map(|c| c.split('-').nth(1).unwrap_or(""))
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            };
            let trimmed = code_content.trim_end_matches('\n');
            format!("```{}\n{}\n```\n\n", lang, trimmed)
        }
        "code" => {
            let content = collect_text(el);
            if content.trim().is_empty() {
                return String::new();
            }
            format!("`{}`", content.trim())
        }
        "a" => {
            let href = el.value().attr("href").unwrap_or("");
            let text = collect_text(el);
            let text = text.trim();
            if text.is_empty() || href.is_empty() {
                text.to_string()
            } else {
                format!("[{}]({})", text, href)
            }
        }
        "img" => {
            let src = el.value().attr("src").unwrap_or("");
            let alt = el.value().attr("alt").unwrap_or("");
            if src.is_empty() {
                String::new()
            } else {
                format!("![{}]({})", alt, src)
            }
        }
        "table" => table_to_markdown(el),
        _ => render_children(el),
    }
}

fn render_children(el: &ElementRef) -> String {
    let mut result = String::new();
    for child in el.children() {
        if let Some(child_el) = ElementRef::wrap(child) {
            result.push_str(&element_to_markdown(&child_el));
        } else if let Node::Text(text) = child.value() {
            let t = text.text.trim();
            if !t.is_empty() {
                result.push_str(t);
            }
        }
    }
    result
}

fn inline_to_markdown(el: &ElementRef) -> String {
    let mut result = String::new();
    for child in el.children() {
        if let Some(child_el) = ElementRef::wrap(child) {
            if is_noise_element(&child_el) {
                continue;
            }
            let tag = child_el.value().name.local.as_ref();
            match tag {
                "a" => {
                    let href = child_el.value().attr("href").unwrap_or("");
                    let text = inline_to_markdown(&child_el);
                    let text = text.trim();
                    if text.is_empty() {
                        continue;
                    }
                    if href.is_empty() {
                        result.push_str(text);
                    } else {
                        result.push_str(&format!("[{}]({})", text, href));
                    }
                }
                "img" => {
                    let src = child_el.value().attr("src").unwrap_or("");
                    let alt = child_el.value().attr("alt").unwrap_or("");
                    if !src.is_empty() {
                        result.push_str(&format!("![{}]({})", alt, src));
                    }
                }
                "strong" | "b" => {
                    let content = inline_to_markdown(&child_el);
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        result.push_str(&format!("**{}**", trimmed));
                    }
                }
                "em" | "i" => {
                    let content = inline_to_markdown(&child_el);
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        result.push_str(&format!("*{}*", trimmed));
                    }
                }
                "code" => {
                    let content = collect_text(&child_el);
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        result.push_str(&format!("`{}`", trimmed));
                    }
                }
                "br" => {
                    result.push_str("  \n");
                }
                _ => {
                    result.push_str(&inline_to_markdown(&child_el));
                }
            }
        } else if let Node::Text(text) = child.value() {
            let t = text.text.trim();
            if !t.is_empty() {
                if !result.is_empty() && !result.ends_with(' ') && !t.starts_with(' ') {
                    result.push(' ');
                }
                result.push_str(t);
            }
        }
    }
    result
}

fn collect_text(el: &ElementRef) -> String {
    let mut result = String::new();
    for child in el.children() {
        if let Node::Text(text) = child.value() {
            result.push_str(text.text.trim());
            if !result.ends_with(' ') {
                result.push(' ');
            }
        } else if let Some(child_el) = ElementRef::wrap(child) {
            result.push_str(&collect_text(&child_el));
        }
    }
    result.trim().to_string()
}

fn table_to_markdown(el: &ElementRef) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();

    for child in el.children() {
        let Some(child_el) = ElementRef::wrap(child) else { continue };
        let tag = child_el.value().name.local.as_ref();
        if tag == "thead" {
            for row_child in child_el.children() {
                if let Some(row_el) = ElementRef::wrap(row_child) {
                    if row_el.value().name.local.as_ref() == "tr" {
                        if let Some(row) = extract_table_row(&row_el) {
                            rows.push(row);
                        }
                    }
                }
            }
        } else if tag == "tbody" {
            for row_child in child_el.children() {
                if let Some(row_el) = ElementRef::wrap(row_child) {
                    if row_el.value().name.local.as_ref() == "tr" {
                        if let Some(row) = extract_table_row(&row_el) {
                            rows.push(row);
                        }
                    }
                }
            }
        } else if tag == "tr" {
            if let Some(row) = extract_table_row(&child_el) {
                rows.push(row);
            }
        }
    }

    if rows.is_empty() {
        return String::new();
    }

    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if col_count == 0 {
        return String::new();
    }

    for row in &mut rows {
        while row.len() < col_count {
            row.push(String::new());
        }
    }

    let mut result = String::new();
    result.push('|');
    for cell in &rows[0] {
        result.push_str(&format!(" {} |", cell));
    }
    result.push('\n');

    result.push('|');
    for _ in 0..col_count {
        result.push_str(" --- |");
    }
    result.push('\n');

    for row in &rows[1..] {
        result.push('|');
        for cell in row {
            result.push_str(&format!(" {} |", cell));
        }
        result.push('\n');
    }

    result.push('\n');
    result
}

fn extract_table_row(el: &ElementRef) -> Option<Vec<String>> {
    let mut cells = Vec::new();
    for child in el.children() {
        if let Some(child_el) = ElementRef::wrap(child) {
            let tag = child_el.value().name.local.as_ref();
            if tag == "td" || tag == "th" {
                let text = collect_text(&child_el);
                cells.push(text);
            }
        }
    }
    if cells.is_empty() { None } else { Some(cells) }
}

pub fn derive_filename(url: &str, title: &str) -> String {
    let path_part = url
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .skip(1)
        .filter(|s| !s.is_empty())
        .last()
        .unwrap_or("")
        .to_string();

    if !path_part.is_empty() {
        let decoded = percent_decode(&path_part);
        let stem = std::path::Path::new(&decoded)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&decoded);
        if !stem.is_empty() && stem != "page" {
            return slugify_filename(stem);
        }
    }

    if !title.is_empty() {
        return slugify_filename(title);
    }

    "page".to_string()
}

fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (
                char::from(bytes[i + 1]).to_digit(16),
                char::from(bytes[i + 2]).to_digit(16),
            ) {
                result.push((hi as u8 * 16 + lo as u8) as char);
                i += 3;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

fn slugify_filename(s: &str) -> String {
    let mut result: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                ' '
            }
        })
        .collect();
    result = result.trim().to_string();
    if result.is_empty() {
        return "page".to_string();
    }
    result = result.replace(' ', "-");
    while result.contains("--") {
        result = result.replace("--", "-");
    }
    result
}

#[allow(dead_code)]
fn html_to_markdown(html: &str) -> String {
    let doc = Html::parse_fragment(html);
    let mut result = String::new();
    for child in doc.root_element().children() {
        if let Some(child_el) = ElementRef::wrap(child) {
            result.push_str(&element_to_markdown(&child_el));
        }
    }

    result = result
        .lines()
        .map(|l| l.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_markdown_basic() {
        let html = r#"<h1>Hello World</h1><p>This is a paragraph.</p>"#;
        let result = html_to_markdown(html);
        assert!(result.contains("# Hello World"));
        assert!(result.contains("This is a paragraph."));
    }

    #[test]
    fn test_html_to_markdown_links() {
        let html = r#"<p>Visit <a href="https://example.com">Example</a>.</p>"#;
        let result = html_to_markdown(html);
        assert!(result.contains("[Example](https://example.com)"));
    }

    #[test]
    fn test_html_to_markdown_images() {
        let html = r#"<img src="https://example.com/image.png" alt="Example Image">"#;
        let result = html_to_markdown(html);
        assert!(result.contains("![Example Image](https://example.com/image.png)"));
    }

    #[test]
    fn test_html_to_markdown_code_block() {
        let html = r#"<pre><code class="language-rust">fn main() {}</code></pre>"#;
        let result = html_to_markdown(html);
        assert!(result.contains("```rust"));
        assert!(result.contains("fn main() {}"));
        assert!(result.contains("```"));
    }

    #[test]
    fn test_html_to_markdown_table() {
        let html = r#"<table><tr><th>Name</th><th>Age</th></tr><tr><td>Alice</td><td>30</td></tr></table>"#;
        let result = html_to_markdown(html);
        assert!(result.contains("| Name | Age |"));
        assert!(result.contains("| Alice | 30 |"));
    }

    #[test]
    fn test_derive_filename_from_url() {
        let name = derive_filename("https://example.com/article", "");
        assert_eq!(name, "article");
    }

    #[test]
    fn test_derive_filename_from_title() {
        let name = derive_filename("https://example.com/page", "My Page Title");
        assert_eq!(name, "My-Page-Title");
    }

    #[test]
    fn test_html_to_markdown_list() {
        let html = r#"<ul><li>Item 1</li><li>Item 2</li></ul>"#;
        let result = html_to_markdown(html);
        assert!(result.contains("- Item 1"));
        assert!(result.contains("- Item 2"));
    }

    #[test]
    fn test_html_to_markdown_blockquote() {
        let html = r#"<blockquote><p>Quote text</p></blockquote>"#;
        let result = html_to_markdown(html);
        assert!(result.contains("> Quote text"));
    }

    #[test]
    fn test_html_to_markdown_inline_code() {
        let html = r#"<p>Use <code>println!</code> macro.</p>"#;
        let result = html_to_markdown(html);
        assert!(result.contains("`println!`"));
    }
}