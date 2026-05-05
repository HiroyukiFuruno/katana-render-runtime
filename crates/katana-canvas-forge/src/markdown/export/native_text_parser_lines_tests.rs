use super::super::native_text::{
    CODE_FENCE_MARKER, HEADING_END_MARKER, HEADING_SEP_MARKER, HEADING_START_MARKER, NativeTextLine,
};
use super::{parse_typed_lines, wrap_line_n};

#[test]
fn parse_typed_lines_handles_code_markers_heading_levels_and_wrapping() {
    let code_blocks = vec![vec![NativeTextLine::code_highlighted(
        "code".to_string(),
        vec![],
    )]];
    let h3 = format!("{HEADING_START_MARKER}h3{HEADING_SEP_MARKER}Title{HEADING_END_MARKER}");
    let h4 = format!("{HEADING_START_MARKER}h4{HEADING_SEP_MARKER}Sub{HEADING_END_MARKER}");
    let h5 = format!("{HEADING_START_MARKER}h5{HEADING_SEP_MARKER}Minor{HEADING_END_MARKER}");
    let h6 = format!("{HEADING_START_MARKER}h6{HEADING_SEP_MARKER}Small{HEADING_END_MARKER}");
    let code = format!("{CODE_FENCE_MARKER}0{CODE_FENCE_MARKER}");
    let long = "word ".repeat(90);
    let text = format!("{h3}\n{h4}\n{h5}\n{h6}\n{code}\n{long}");
    let lines = parse_typed_lines(&text, &code_blocks);

    assert!(lines.iter().any(|line| line.is_code));
    assert!(lines.iter().any(|line| line.text == "Title"));
    assert!(lines.len() > 4);
}

#[test]
fn parse_typed_lines_ignores_malformed_markers() {
    let malformed = format!(
        "{CODE_FENCE_MARKER}bad{CODE_FENCE_MARKER}\n{HEADING_START_MARKER}bad\n{HEADING_START_MARKER}h9{HEADING_SEP_MARKER}Bad{HEADING_END_MARKER}"
    );
    let lines = parse_typed_lines(&malformed, &[]);

    assert!(!lines.is_empty());
}

#[test]
fn parse_typed_lines_keeps_single_short_body_line() {
    let lines = parse_typed_lines("short", &[]);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].text, "short");
}

#[test]
fn parse_typed_lines_keeps_empty_input_empty() {
    assert!(parse_typed_lines("", &[]).is_empty());
}

#[test]
fn wrap_line_n_returns_no_rows_for_empty_input() {
    assert!(wrap_line_n("", 10).is_empty());
}

#[test]
fn wrap_line_n_keeps_single_long_word_without_empty_prefix() {
    let long_word = "x".repeat(20);

    assert_eq!(wrap_line_n(&long_word, 10), vec![long_word]);
}
