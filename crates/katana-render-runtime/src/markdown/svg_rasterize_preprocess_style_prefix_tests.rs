use super::normalize_inline_style_prefixes;

const SVG_START: &str = r#"<svg xmlns="http://www.w3.org/2000/svg">"#;
const SVG_END: &str = "</svg>";
const STYLE_CASES: [(&str, &str, &str); 8] = [
    (
        "single empty declaration",
        "; font-weight:bold",
        " font-weight:bold",
    ),
    (
        "comments whitespace and multiple empties",
        " /*先行*/ ;; font-weight:bold",
        " /*先行*/  font-weight:bold",
    ),
    (
        "quoted semicolon and important",
        ";; content:'a;b'; font-weight:bold !important",
        " content:&apos;a;b&apos;; font-weight:bold !important",
    ),
    (
        "entity encoded semicolon",
        "&#59; font-weight:bold",
        " font-weight:bold",
    ),
    (
        "unicode value",
        "; font-family:'日本語'",
        " font-family:&apos;日本語&apos;",
    ),
    (
        "ordinary no-semicolon input",
        "font-weight:bold",
        "font-weight:bold",
    ),
    (
        "ordinary declaration semicolon",
        "font-weight:bold;fill:red",
        "font-weight:bold;fill:red",
    ),
    ("only empty declarations", ";;", ";;"),
];

fn normalized_svg(input: &str) -> String {
    normalize_inline_style_prefixes(input).unwrap_or_else(|| input.to_owned())
}

fn svg_with_style(style: &str) -> String {
    format!(r#"{SVG_START}<text style="{style}"/>{SVG_END}"#)
}

#[test]
fn preserves_expected_style_prefix_cases() {
    for (case, input_style, expected_style) in STYLE_CASES {
        assert_eq!(
            normalized_svg(&svg_with_style(input_style)),
            svg_with_style(expected_style),
            "{case}"
        );
    }
}

#[test]
fn does_not_change_style_on_non_svg_element() {
    let input = format!(
        r#"{SVG_START}<html:div xmlns:html="http://www.w3.org/1999/xhtml" style="; font-weight:bold"/>{SVG_END}"#
    );
    assert_eq!(normalized_svg(&input), input);
}

#[test]
fn normalizes_svg_without_namespace() {
    let input = r#"<svg><text style="; font-weight:bold"/></svg>"#;
    let expected = r#"<svg><text style=" font-weight:bold"/></svg>"#;
    assert_eq!(normalized_svg(input), expected);
}
