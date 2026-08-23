//! Markdown post-processing pipeline.
//!
//! Stages: Normalize → Cleanup. Each stage takes a markdown string and
//! returns a cleaned/transformed one.

/// Full pipeline: normalize → cleanup.
pub fn process(markdown: &str) -> String {
    let normalized = normalize(markdown);
    cleanup(&normalized)
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

        // Skip page break markers (form feed character only). Note: a plain
        // `---` horizontal rule must NOT be removed here, otherwise legitimate
        // `<hr>` output and other `---` rules are lost.
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
    // A markdown table separator line must contain at least one `|`; without it
    // a run of dashes under a `|`-less line is not a real table separator.
    if !trimmed.contains('|') {
        return false;
    }
    if !trimmed.contains('-') {
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
// Tests
// ---------------------------------------------------------------------------

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
    fn process_normalizes_and_cleans_up() {
        let input = "# Title\n\nSome content\n\n\n\nMore content";
        let result = process(input);
        assert!(result.contains("# Title"));
        assert!(result.contains("Some content"));
        assert!(result.contains("More content"));
    }

    #[test]
    fn count_table_separators_counts_only_real_separators() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |\n\ncode:\n```\n|---|\n```\n| x | y |\n|---|---|";
        assert_eq!(count_table_separators(input), 2);
    }

    #[test]
    fn cleanup_keeps_horizontal_rule() {
        // A horizontal rule (---) preceded by a blank line must be preserved,
        // not deleted as a page-break artifact.
        let input = "text\n\n---\n\nmore text";
        let result = cleanup(input);
        assert!(result.contains("---"));
        assert!(result.contains("text"));
        assert!(result.contains("more text"));
    }

    #[test]
    fn is_table_separator_requires_pipe() {
        assert!(!is_table_separator("----"));
        assert!(!is_table_separator("===="));
        // A bare `---` with no pipe is not a valid GFM delimiter row.
        assert!(!is_table_separator("---"));
        assert!(is_table_separator("|---|---|"));
    }
}
