use super::valid_span_range;

#[test]
fn utf8_span_ranges_require_nonempty_character_boundaries() {
    let text = "é A";
    assert!(valid_span_range(text, 0, 2));
    assert!(valid_span_range(text, 2, text.len()));
    assert!(!valid_span_range(text, 0, 1));
    assert!(!valid_span_range(text, 1, 2));
    assert!(!valid_span_range(text, 2, 2));
    assert!(!valid_span_range(text, 0, text.len() + 1));
}
