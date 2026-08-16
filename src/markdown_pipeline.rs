//! Markdown post-processing pipeline.
//!
//! Stages: Normalize → Cleanup → Output Mode Formatter
//! Each stage takes a markdown string and returns a cleaned/transformed one.

use crate::models::task::{AiReadyOpts, OutputMode};

/// Full pipeline: normalize → cleanup → apply output mode.
pub fn process(
    markdown: &str,
    mode: &OutputMode,
    source_path: &str,
    opts: &AiReadyOpts,
) -> String {
    let normalized = normalize(markdown);
    let cleaned = cleanup(&normalized);
    apply_output_mode(&cleaned, mode, source_path, opts)
}

/// Count markdown table separator rows (e.g. `|---|---|`), ignoring code
/// blocks. Used for statistics.
pub fn count_table_separators(markdown: &str) -> usize {
    let mut count = 0;
    let mut in_code_block = false;
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        if is_table_separator(line) {
            count += 1;
        }
    }
    count
}

/// Count image references (`![alt](path)`) in the markdown, ignoring code
/// blocks. Used for statistics and to verify assets were bundled.
pub fn count_images(markdown: &str) -> usize {
    let mut count = 0;
    let mut in_code_block = false;
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        count += line.matches("![").count();
    }
    count
}

/// Count words in the markdown, ignoring code blocks and inline code.
///
/// For CJK scripts (Han, Hiragana, Katakana, Hangul) each character counts as
/// one "word"; for Latin/other scripts runs of alphanumeric characters count as
/// one word. This mirrors how word processors count CJK documents.
pub fn count_words(markdown: &str) -> usize {
    let mut count = 0usize;
    let mut in_code_block = false;
    let mut in_inline_code = false;
    let mut buf = String::with_capacity(16);

    let flush = |buf: &mut String, count: &mut usize| {
        if !buf.is_empty() {
            *count += buf.split_whitespace().filter(|w| !w.is_empty()).count();
            buf.clear();
        }
    };

    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '`' {
                // toggle inline code; flush pending text first
                flush(&mut buf, &mut count);
                in_inline_code = !in_inline_code;
                continue;
            }
            if in_inline_code {
                continue;
            }
            if is_cjk(c) {
                flush(&mut buf, &mut count);
                count += 1;
            } else {
                buf.push(c);
            }
        }
        // end of line: treat as whitespace boundary
        buf.push(' ');
    }
    flush(&mut buf, &mut count);
    count
}

/// Whether a character belongs to a CJK script that should be counted
/// per-character rather than per-whitespace-delimited word.
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'      // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}'    // CJK Extension A
        | '\u{F900}'..='\u{FAFF}'    // CJK Compatibility Ideographs
        | '\u{3040}'..='\u{309F}'    // Hiragana
        | '\u{30A0}'..='\u{30FF}'    // Katakana
        | '\u{AC00}'..='\u{D7AF}'    // Hangul Syllables
        | '\u{FF00}'..='\u{FFEF}'    // Fullwidth forms
    )
}

// ---------------------------------------------------------------------------
// Stage 1: Normalize
// ---------------------------------------------------------------------------

/// Fix heading levels (no skipping), normalize list markers, trim whitespace.
pub fn normalize(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut result = String::with_capacity(markdown.len());

    // Track heading levels to prevent skipping (e.g. H1 → H3 becomes H1 → H2).
    let mut heading_remap: std::collections::HashMap<u8, u8> = std::collections::HashMap::new();
    let mut next_level: u8 = 0;

    // First pass: build remap table by scanning headings in order.
    for line in &lines {
        if let Some(level) = parse_heading_level(line) {
            if !heading_remap.contains_key(&level) {
                next_level = (next_level + 1).min(6);
                heading_remap.insert(level, next_level);
            }
        }
    }

    // Reset for second pass.
    let mut in_code_block = false;

    for line in &lines {
        // Track code fence state — don't touch content inside code blocks.
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            result.push_str(line);
            result.push('\n');
            continue;
        }

        if in_code_block {
            result.push_str(line);
            result.push('\n');
            continue;
        }

        // Remap heading levels.
        if let Some(level) = parse_heading_level(line) {
            let new_level = *heading_remap.get(&level).unwrap_or(&level);
            let rest = &line[level as usize..];
            for _ in 0..new_level {
                result.push('#');
            }
            result.push_str(rest);
            result.push('\n');
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Trim trailing newline that the loop always adds.
    if result.ends_with('\n') {
        result.pop();
    }

    result
}

/// Extract the heading level from a markdown heading line (e.g. `### Title` → 3).
/// Returns None if not a heading.
fn parse_heading_level(line: &str) -> Option<u8> {
    let trimmed = line.trim_start();
    let mut count = 0;
    for c in trimmed.chars() {
        if c == '#' {
            count += 1;
        } else {
            break;
        }
    }
    if count > 0 && count <= 6 {
        let after = &trimmed[count..];
        if after.starts_with(' ') || after.is_empty() {
            return Some(count as u8);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Stage 2: Cleanup
// ---------------------------------------------------------------------------

/// Remove blank lines, empty tables, page breaks, garbled characters.
pub fn cleanup(markdown: &str) -> String {
    let mut result = String::with_capacity(markdown.len());
    let mut blank_count = 0;
    let mut in_code_block = false;

    for line in markdown.lines() {
        // Track code fence state.
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            blank_count = 0;
            result.push_str(line);
            result.push('\n');
            continue;
        }

        if in_code_block {
            blank_count = 0;
            result.push_str(line);
            result.push('\n');
            continue;
        }

        let trimmed = line.trim();

        // Skip page break markers.
        if trimmed == "---" && blank_count > 0 {
            continue;
        }
        if trimmed.contains('\u{000c}') {
            // Form feed character (page break).
            let cleaned = trimmed.replace('\u{000c}', "");
            if cleaned.is_empty() {
                continue;
            }
            blank_count = 0;
            result.push_str(&cleaned);
            result.push('\n');
            continue;
        }

        // Collapse consecutive blank lines to max 1.
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count <= 1 {
                result.push('\n');
            }
            continue;
        }

        blank_count = 0;
        result.push_str(line);
        result.push('\n');
    }

    // Strip trailing whitespace.
    let result = result.trim_end().to_string();

    // Remove empty markdown tables (header + separator only, no data rows).
    remove_empty_tables(&result)
}

/// Remove tables that have a header row and separator but no data rows,
/// or tables where all data cells are empty.
fn remove_empty_tables(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut result = String::with_capacity(markdown.len());
    let mut i = 0;

    while i < lines.len() {
        // Detect a markdown table: a line with `|` followed by a separator line.
        if i + 1 < lines.len()
            && lines[i].contains('|')
            && is_table_separator(lines[i + 1])
        {
            // Collect all table lines.
            let table_start = i;
            i += 2; // skip header + separator
            while i < lines.len() && lines[i].trim().starts_with('|') {
                i += 1;
            }

            let table_lines = &lines[table_start..i];
            // A table with only header + separator (no data rows) is empty.
            // Also check if all data cells are empty.
            if table_lines.len() <= 2 || table_all_empty(table_lines) {
                // Skip this table (don't copy it).
                continue;
            } else {
                for line in table_lines {
                    result.push_str(line);
                    result.push('\n');
                }
                continue;
            }
        }

        result.push_str(lines[i]);
        result.push('\n');
        i += 1;
    }

    result.trim_end().to_string()
}

fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('|') && !trimmed.contains('-') {
        return false;
    }
    // A separator line looks like: |---|---| or | --- | --- |
    trimmed
        .chars()
        .all(|c| c == '|' || c == '-' || c == ':' || c == ' ')
}

fn table_all_empty(lines: &[&str]) -> bool {
    if lines.len() <= 2 {
        return true;
    }
    // Check data rows (skip header + separator).
    lines[2..]
        .iter()
        .all(|line| {
            line.split('|')
                .filter(|s| !s.is_empty())
                .all(|s| s.trim().is_empty())
        })
}

// ---------------------------------------------------------------------------
// Stage 3: Output Mode Formatter
// ---------------------------------------------------------------------------

/// Apply output-mode-specific transformations.
fn apply_output_mode(
    markdown: &str,
    mode: &OutputMode,
    source_path: &str,
    opts: &AiReadyOpts,
) -> String {
    match mode {
        OutputMode::Standard => markdown.to_string(),
        OutputMode::AiReady => format_ai_ready(markdown, opts, source_path),
        OutputMode::Obsidian => format_obsidian(markdown, source_path),
    }
}

/// AI Ready: clean, structured, suitable for LLM reading.
///
/// Safe, deterministic transforms (no external calls, no heuristics that risk
/// dropping real content):
/// - Strip residual inline HTML tags (content preserved), leaving code blocks
///   untouched.
/// - Collapse runs of consecutive identical non-empty lines (headings exempt).
/// - Ensure a single H1: keep the first `# ` heading, demote any later H1 to H2.
/// - Optionally prepend a generated table of contents (`opts.gen_toc`).
/// - Optionally prepend a controlled metadata comment block (`opts.gen_meta`).
fn format_ai_ready(markdown: &str, opts: &AiReadyOpts, source_path: &str) -> String {
    let stripped = strip_residual_html(markdown);
    let deduped = dedupe_consecutive_lines(&stripped);
    let single_h1 = ensure_single_h1(&deduped);

    let mut out = String::with_capacity(single_h1.len() + 256);

    if opts.gen_meta {
        out.push_str(&render_meta_comment(source_path));
        out.push_str("\n\n");
    }

    if opts.gen_toc {
        let toc = render_toc(&single_h1);
        if !toc.is_empty() {
            out.push_str(&toc);
            out.push_str("\n\n");
        }
    }

    out.push_str(&single_h1);
    out
}

/// Remove residual HTML tags (`<tag ...>`, `</tag>`, self-closing) while
/// preserving their inner text. Code blocks are left untouched.
fn strip_residual_html(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut in_code_block = false;

    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_code_block {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        out.push_str(&strip_html_from_line(line));
        out.push('\n');
    }

    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Strip HTML tags from a single line, preserving inner text. A "tag" starts
/// with `<` optionally followed by `/`, then a letter, and ends at the next
/// `>`. Anything outside tags is kept verbatim.
fn strip_html_from_line(line: &str) -> String {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < len {
        if bytes[i] == b'<' {
            // Peek: optional '/' then a letter means this looks like an HTML tag.
            let mut j = i + 1;
            if j < len && bytes[j] == b'/' {
                j += 1;
            }
            if j < len && bytes[j].is_ascii_alphabetic() {
                // Scan to the closing '>'.
                let mut end = j;
                while end < len && bytes[end] != b'>' {
                    end += 1;
                }
                if end < len {
                    // Drop the whole tag (i..=end).
                    i = end + 1;
                    continue;
                }
                // No closing '>' on this line — keep the '<' as literal text.
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Collapse consecutive identical non-empty lines into one. Heading lines
/// (starting with `#`) and code-block content are never merged.
fn dedupe_consecutive_lines(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut in_code_block = false;
    let mut last_kept: Option<String> = None;

    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            out.push_str(line);
            out.push('\n');
            last_kept = Some(line.to_string());
            continue;
        }
        if in_code_block {
            out.push_str(line);
            out.push('\n');
            last_kept = Some(line.to_string());
            continue;
        }

        let is_heading = line.trim_start().starts_with('#');
        let trimmed = line.trim();
        if !is_heading && !trimmed.is_empty() {
            if let Some(prev) = &last_kept {
                if prev.trim() == trimmed {
                    // Duplicate of the previous kept line — skip.
                    continue;
                }
            }
        }
        out.push_str(line);
        out.push('\n');
        last_kept = Some(line.to_string());
    }

    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Ensure the document has at most one H1. The first `# ` heading is kept;
/// any subsequent H1 is demoted to H2. Other heading levels are unchanged.
fn ensure_single_h1(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut in_code_block = false;
    let mut seen_h1 = false;

    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_code_block {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if let Some(level) = parse_heading_level(line) {
            if level == 1 {
                if seen_h1 {
                    // Demote this H1 to H2 by prepending one extra '#'.
                    out.push('#');
                    out.push_str(line);
                    out.push('\n');
                    continue;
                }
                seen_h1 = true;
            }
        }
        out.push_str(line);
        out.push('\n');
    }

    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Build a `## 目录` block from H1/H2/H3 headings. Anchors use GitHub-style
/// slugs; non-sluggable text (e.g. CJK) falls back to `section-N`.
fn render_toc(markdown: &str) -> String {
    let mut entries: Vec<(u8, String)> = Vec::new();
    let mut in_code_block = false;
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        if let Some(level) = parse_heading_level(line) {
            if level >= 1 && level <= 3 {
                let text = line.trim_start()[level as usize..].trim().to_string();
                if !text.is_empty() {
                    entries.push((level, text));
                }
            }
        }
    }

    if entries.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str("## 目录\n");
    for (level, text) in &entries {
        let slug = slugify(text);
        let indent = "  ".repeat((level.saturating_sub(1)) as usize);
        out.push_str(&format!("{}- [{}](#{})\n", indent, text, slug));
    }
    // Trim trailing newline.
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn slugify(text: &str) -> String {
    let mut slug: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else if c.is_whitespace() { '-' } else { '\0' })
        .collect();
    // Collapse multiple dashes.
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        // Fall back to a stable placeholder for non-latin headings.
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return format!("section-{}", n);
    }
    slug
}

/// Render a controlled, comment-style metadata block. Keeping it inside an
/// HTML comment means it is invisible in rendered markdown but still available
/// to LLM/RAG pipelines that read the raw text.
fn render_meta_comment(source_path: &str) -> String {
    let source_label = std::path::Path::new(source_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(source_path);
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    format!(
        "<!-- omnimd:meta\n  source: {}\n  converted: {}\n  mode: ai-ready\n-->",
        source_label, now
    )
}


/// Obsidian: YAML frontmatter + relative asset paths.
fn format_obsidian(markdown: &str, source_path: &str) -> String {
    let title = std::path::Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    let created = {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let dt = chrono::DateTime::from_timestamp(now as i64, 0)
            .unwrap_or_default();
        dt.format("%Y-%m-%d").to_string()
    };

    let source_url = if source_path.starts_with("http://") || source_path.starts_with("https://") {
        source_path.to_string()
    } else {
        String::new()
    };

    let mut frontmatter = String::new();
    frontmatter.push_str("---\n");
    frontmatter.push_str(&format!("title: {}\n", escape_yaml(&title)));
    if !source_url.is_empty() {
        frontmatter.push_str(&format!("source: {}\n", escape_yaml(&source_url)));
    }
    frontmatter.push_str(&format!("created: {}\n", created));
    frontmatter.push_str("tags:\n  - imported\n");
    frontmatter.push_str("---\n\n");

    format!("{}{}", frontmatter, markdown)
}

fn escape_yaml(s: &str) -> String {
    // Quote if it contains special characters.
    if s.contains(':') || s.contains('#') || s.contains('\n') || s.starts_with(' ') {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_heading_levels() {
        let input = "# Title\n\n#### Sub\n\n## Section\n\n### Deep\n\n### Deep2";
        let result = normalize(input);
        // H1→H1, H4→H2, H2→H3, H3→H4, H3→H4
        assert!(result.contains("# Title"));
        assert!(result.contains("## Sub"));
        assert!(result.contains("### Section"));
        assert!(result.contains("#### Deep"));
        assert!(result.contains("#### Deep2"));
    }

    #[test]
    fn cleanup_collapses_blank_lines() {
        let input = "line1\n\n\n\n\nline2";
        let result = cleanup(input);
        assert_eq!(result, "line1\n\nline2");
    }

    #[test]
    fn cleanup_removes_page_breaks() {
        let input = "line1\n\u{000c}\nline2";
        let result = cleanup(input);
        assert!(!result.contains('\u{000c}'));
    }

    #[test]
    fn cleanup_removes_empty_tables() {
        let input = "text\n\n| A | B |\n|---|---|\n\nmore text";
        let result = cleanup(input);
        assert!(!result.contains("| A | B |"));
        assert!(result.contains("text"));
    }

    #[test]
    fn cleanup_keeps_nonempty_tables() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |\n";
        let result = cleanup(input);
        assert!(result.contains("| A | B |"));
        assert!(result.contains("| 1 | 2 |"));
    }

    #[test]
    fn obsidian_format_adds_frontmatter() {
        let input = "# Hello\n\nWorld";
        let result = format_obsidian(input, "/tmp/report.pdf");
        assert!(result.starts_with("---\n"));
        assert!(result.contains("title: report"));
        assert!(result.contains("tags:"));
        assert!(result.contains("- imported"));
        assert!(result.contains("# Hello"));
    }

    #[test]
    fn process_ai_ready_preserves_content() {
        let input = "# Title\n\nSome content\n\n\n\nMore content";
        let result = process(input, &OutputMode::AiReady, "/tmp/test.pdf", &AiReadyOpts::default());
        assert!(result.contains("# Title"));
        assert!(result.contains("Some content"));
        assert!(result.contains("More content"));
        assert!(!result.contains("---\n")); // No frontmatter in AI Ready
    }

    #[test]
    fn strip_residual_html_removes_tags_keeps_text() {
        let input = "Hello <b>world</b> and <img src=\"x.png\"> bye";
        let result = strip_residual_html(input);
        assert!(!result.contains('<'));
        assert!(result.contains("Hello"));
        assert!(result.contains("world"));
        assert!(result.contains("bye"));
    }

    #[test]
    fn strip_residual_html_leaves_code_blocks_alone() {
        let input = "```\n<div>keep me</div>\n```\n";
        let result = strip_residual_html(input);
        assert!(result.contains("<div>keep me</div>"));
    }

    #[test]
    fn dedupe_collapses_consecutive_identical_lines() {
        let input = "line1\nline1\nline1\nline2\nline2";
        let result = dedupe_consecutive_lines(input);
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn dedupe_never_merges_headings() {
        let input = "# Title\n# Title\n## Sub\n## Sub";
        let result = dedupe_consecutive_lines(input);
        // Both headings survive.
        assert_eq!(result.matches("# Title").count(), 2);
        assert_eq!(result.matches("## Sub").count(), 2);
    }

    #[test]
    fn ensure_single_h1_demotes_extra_h1() {
        let input = "# First\n\nbody\n\n# Second\n\n# Third";
        let result = ensure_single_h1(input);
        assert_eq!(result.matches("# First").count(), 1);
        assert!(result.contains("## Second"));
        assert!(result.contains("## Third"));
    }

    #[test]
    fn ai_ready_with_toc_generates_toc() {
        let opts = AiReadyOpts {
            gen_toc: true,
            gen_meta: false,
        };
        let input = "# Title\n\n## Section A\n\ntext\n\n## Section B";
        let result = format_ai_ready(input, &opts, "/tmp/x.pdf");
        assert!(result.contains("## 目录"));
        assert!(result.contains("[Title](#"));
        assert!(result.contains("[Section A](#"));
        assert!(result.contains("[Section B](#"));
    }

    #[test]
    fn ai_ready_with_meta_generates_comment() {
        let opts = AiReadyOpts {
            gen_toc: false,
            gen_meta: true,
        };
        let input = "# Title\nbody";
        let result = format_ai_ready(input, &opts, "/tmp/report.pdf");
        assert!(result.starts_with("<!-- omnimd:meta"));
        assert!(result.contains("source: report.pdf"));
        assert!(result.contains("converted:"));
        assert!(result.contains("# Title"));
    }

    #[test]
    fn ai_ready_without_opts_does_not_add_extras() {
        let opts = AiReadyOpts::default();
        let input = "# Title\nbody";
        let result = format_ai_ready(input, &opts, "/tmp/x.pdf");
        assert!(!result.contains("omnimd:meta"));
        assert!(!result.contains("## 目录"));
    }

    #[test]
    fn count_table_separators_counts_only_real_separators() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |\n\ncode:\n```\n|---|\n```\n| x | y |\n|---|---|";
        assert_eq!(count_table_separators(input), 2);
    }
}
