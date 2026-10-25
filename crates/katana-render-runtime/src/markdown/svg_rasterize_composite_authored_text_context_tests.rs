use super::authored_text_context::collect;
use resvg::usvg::roxmltree::Document;

const LABEL: &str = "Runtime";
const ACCEPTED_KEYFRAMES: &str = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>
.ishikawa text { font-size: 16px; }
.ishikawa .label { dominant-baseline: middle; }
@keyframes edge-animation-frame { 0% { stroke-dashoffset: 0; } to { stroke-dashoffset: 1; } }
</style><g class="ishikawa"><text class="label"><tspan dy="0">Runtime</tspan></text></g></svg>"#;

const CASES: &[(&str, bool)] = &[
    (ACCEPTED_KEYFRAMES, true),
    (
        r#"<svg><style>#svg :root { --mermaid-font-family: Trebuchet; }</style><text><tspan dy="0">Runtime</tspan></text></svg>"#,
        true,
    ),
    (
        r#"<svg><style>.label/*ok*/[href="https://example.test/a:b|c"] { font-size: 16px; }</style><text class="label" href="https://example.test/a:b|c"><tspan dy="0">Runtime</tspan></text></svg>"#,
        true,
    ),
    (
        r#"<svg><style>#svg :root { font-size: 16px; }</style><text><tspan dy="0">Runtime</tspan></text></svg>"#,
        false,
    ),
    (
        r#"<svg><style>text { font-family: var(--mermaid-font-family); }</style><text><tspan dy="0">Runtime</tspan></text></svg>"#,
        false,
    ),
    (
        r#"<svg><style>.label { color: red;; FONT-SIZE: 18px; }</style><text class="label"><tspan dy="0">Runtime</tspan></text></svg>"#,
        true,
    ),
    (
        r#"<svg><style>.label { color: red;; FONT-SIZE: 18px; }</style><text><tspan class="label" dy="0">Runtime</tspan></text></svg>"#,
        false,
    ),
    (
        r#"<svg><style>.label /invalid { font-size: 18px; }</style><text class="label"><tspan dy="0">Runtime</tspan></text></svg>"#,
        false,
    ),
    (
        r#"<svg><style>.label { font: 16px var(--mermaid-font-family); }</style><text class="label"><tspan dy="0">Runtime</tspan></text></svg>"#,
        false,
    ),
    (
        r#"<svg><style>@keyframes k { 0% { font-weight: bold; } }</style><text><tspan dy="0">Runtime</tspan></text></svg>"#,
        false,
    ),
    (
        r#"<svg><text><tspan dy="0">Runtime</tspan>outside</text></svg>"#,
        false,
    ),
    (
        r#"<svg><text><tspan dy="0" font-size="18">Runtime</tspan></text></svg>"#,
        false,
    ),
    (
        r#"<svg><text><tspan dy="0" style="color:red;;FONT-SIZE:18px">Runtime</tspan></text></svg>"#,
        false,
    ),
    (
        r#"<svg><style>@media screen { .label { font-size:18px; } }</style><text class="label"><tspan dy="0">Runtime</tspan></text></svg>"#,
        false,
    ),
    (
        r#"<svg><text><tspan dy="0">Runtime</tspan></text><text><tspan dy="0">Runtime</tspan></text></svg>"#,
        false,
    ),
    (r#"<svg><text>Runtime</text></svg>"#, false),
    (
        r#"<svg><defs><text><tspan dy="0">Runtime</tspan></text></defs></svg>"#,
        false,
    ),
    (
        r#"<svg><text><tspan dy="0">Run</tspan><tspan dy="0">time</tspan></text></svg>"#,
        false,
    ),
    (
        r#"<svg><text><tspan dy="0">Runtime</tspan></text><use href='#run'/></svg>"#,
        false,
    ),
];
#[test]
fn nested_single_line_context_requires_unambiguous_authored_structure() -> Result<(), String> {
    for &(source, expected) in CASES {
        assert_eq!(context_matches(source, LABEL)?, expected, "{source}");
    }
    assert!(!context_matches(ACCEPTED_KEYFRAMES, "Runtime ")?);
    Ok(())
}

fn context_matches(source: &str, text: &str) -> Result<bool, String> {
    let document = Document::parse(source).map_err(|error| error.to_string())?;
    Ok(collect(&document).matches_text(text))
}
