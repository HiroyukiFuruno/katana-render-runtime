use super::{css_resolved_px, split_top_level_commas, split_top_level_whitespace};

#[test]
fn relative_lengths_keep_decimal_precision_until_resolution() {
    assert_eq!(
        css_resolved_px("0.84em", 14.0, 100.0, 50.0, false),
        Some(11.76)
    );
    assert_eq!(
        css_resolved_px("0.84rem", 14.0, 100.0, 50.0, false),
        Some(13.44)
    );
    assert!(css_resolved_px("1e100px", 14.0, 100.0, 50.0, false).is_none());
}

#[test]
fn math_function_rejects_unresolvable_nested_values() {
    assert!(css_resolved_px("min(1px, invalid)", 16.0, 100.0, 50.0, false).is_none());
}

#[test]
fn top_level_comma_split_rejects_empty_and_unbalanced_arguments() {
    assert!(split_top_level_commas(", 1px").is_none());
    assert!(split_top_level_commas("min(1px, 2px").is_none());
    assert!(split_top_level_commas("1px,").is_none());
}

#[test]
fn top_level_whitespace_split_preserves_nested_parentheses() {
    assert_eq!(
        split_top_level_whitespace("calc(1px + 2px) 3px"),
        vec!["calc(1px + 2px)", "3px"]
    );
}

#[test]
fn top_level_whitespace_split_returns_final_token_without_trailing_whitespace() {
    assert_eq!(split_top_level_whitespace("  5px  "), vec!["5px"]);
}
