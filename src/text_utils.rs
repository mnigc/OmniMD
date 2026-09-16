//! Shared text predicates used by both the Markdown pipeline (word counting)
//! and the search index (CJK bigram tokenization), so the two never disagree
//! about what counts as a CJK character.

/// Whether `c` belongs to a CJK / CJK-adjacent script.
///
/// Covers Han ideographs (Unified, Extensions A/B/C–F/G and compatibility
/// forms), Japanese kana, and Korean Hangul (syllables and Jamo).
pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11FF      // Hangul Jamo
        | 0x3040..=0x30FF    // Hiragana + Katakana
        | 0x31F0..=0x31FF    // Katakana Phonetic Extensions
        | 0x3400..=0x4DBF    // CJK Unified Ideographs Extension A
        | 0x4E00..=0x9FFF    // CJK Unified Ideographs
        | 0xAC00..=0xD7AF    // Hangul Syllables
        | 0xF900..=0xFAFF    // CJK Compatibility Ideographs
        | 0x20000..=0x2A6DF  // CJK Unified Ideographs Extension B
        | 0x2A700..=0x2EBEF  // CJK Unified Ideographs Extensions C–F
        | 0x2F800..=0x2FA1F  // CJK Compatibility Ideographs Supplement
        | 0x30000..=0x3134F  // CJK Unified Ideographs Extension G
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cjk_and_rejects_ascii() {
        assert!(is_cjk('中'));
        assert!(is_cjk('あ'));
        assert!(is_cjk('한'));
        assert!(!is_cjk('a'));
        assert!(!is_cjk('1'));
        assert!(!is_cjk('，')); // fullwidth punctuation is not a CJK ideograph
    }
}
