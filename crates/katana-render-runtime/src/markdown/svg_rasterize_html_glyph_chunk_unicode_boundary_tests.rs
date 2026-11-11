use super::span_content;

#[test]
fn utf8_span_ranges_require_nonempty_character_boundaries() {
    let text = "é A";
    assert_eq!(span_content(text, 0, 2), Some("é"));
    assert_eq!(span_content(text, 2, text.len()), Some(" A"));
    assert!(span_content(text, 0, 1).is_none());
    assert!(span_content(text, 1, 2).is_none());
    assert!(span_content(text, 2, 2).is_none());
    assert!(span_content(text, 0, text.len() + 1).is_none());
}
