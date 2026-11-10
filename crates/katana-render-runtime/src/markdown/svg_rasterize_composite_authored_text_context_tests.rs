use super::authored_text_context::collect;
use super::{GeneratedClip, GeneratedClipProvenance, authored_svg_context};
use resvg::usvg::roxmltree::Document;

const LABEL: &str = "Runtime";
const AUTHORED_FRAME_SIZE: (u32, u32) = (64, 32);
const RENDERED_AUTHORED_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"><style>
.label { font-size: 16px; dominant-baseline: middle; }
</style><text class="label" x="2" y="16"><tspan dy="0">Runtime</tspan></text></svg>"#;
const INLINE_AUTHORED_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"><text font-size="16px" dominant-baseline="middle" x="2" y="16"><tspan dy="0">Runtime</tspan></text></svg>"#;
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

#[test]
fn surrounding_whitespace_cannot_prove_exact_authored_text() -> Result<(), String> {
    let source = "<svg><text> \n<tspan dy='0'>Runtime</tspan>\n </text></svg>";
    assert!(context_matches(INLINE_AUTHORED_SVG, LABEL)?);
    assert!(!context_matches(source, LABEL)?);
    Ok(())
}

#[test]
fn css_styled_and_inline_authored_text_render_identically() -> Result<(), String> {
    let document = Document::parse(RENDERED_AUTHORED_SVG).map_err(|error| error.to_string())?;
    assert!(collect(&document).matches_text(LABEL));
    assert_same_visible_render(RENDERED_AUTHORED_SVG, INLINE_AUTHORED_SVG)
}

#[test]
fn single_line_proof_does_not_match_first_chunk_of_multiline_text() -> Result<(), String> {
    const MULTILINE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="72">
        <text id="candidate" x="2" y="16" font-size="16"><tspan>Runtime</tspan><tspan x="2" dy="20">Continuation</tspan></text>
    </svg>"#;
    let document = Document::parse(INLINE_AUTHORED_SVG).map_err(|error| error.to_string())?;
    let context = collect(&document);
    assert!(context.matches_text(LABEL));

    let options = super::super::rasterizer_options();
    let tree =
        resvg::usvg::Tree::from_str(MULTILINE, &options).map_err(|error| error.to_string())?;
    let Some(resvg::usvg::Node::Text(text)) = tree.node_by_id("candidate") else {
        return Err("multiline text is missing".into());
    };
    let [first, second] = text.chunks() else {
        return Err("multiline input did not produce two chunks".into());
    };
    assert_eq!(first.text(), LABEL);
    assert_eq!(second.text(), "Continuation");
    assert!(!context.matches(text));
    Ok(())
}

#[test]
fn inert_styles_preserve_nested_text_rendering() -> Result<(), String> {
    let candidate = INLINE_AUTHORED_SVG.replace(
        "<text ",
        "<style type='text/plain'>tspan { font-size: 28px; }</style><style/><text ",
    );
    assert!(context_matches(&candidate, LABEL)?);
    assert_same_visible_render(&candidate, INLINE_AUTHORED_SVG)
}

fn assert_same_visible_render(candidate: &str, control: &str) -> Result<(), String> {
    let render = |svg| {
        crate::markdown::svg_rasterize::SvgRasterizeOps::rasterize_html_svg(svg, 1.0)
            .map_err(|error| error.to_string())
    };
    let candidate = render(candidate)?;
    let control = render(control)?;
    for frame in [&candidate, &control] {
        assert_eq!((frame.width, frame.height), AUTHORED_FRAME_SIZE);
    }
    assert_eq!(candidate.rgba, control.rgba);
    assert!(control.rgba.chunks_exact(4).any(|pixel| pixel[3] != 0));
    Ok(())
}

#[test]
fn authored_context_keeps_clip_provenance_and_text_together() -> Result<(), String> {
    let shape = r#"<rect width="1" height="1"/>"#;
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><defs>
        <clipPath id="authored" clipPathUnits="objectBoundingBox">{shape}</clipPath>
        <clipPath id="generated">{shape}</clipPath>
        </defs><text x="2" y="16"><tspan dy="0">{LABEL}</tspan></text></svg>"#
    );
    let generated = GeneratedClipProvenance {
        clips: vec![GeneratedClip {
            id: "generated".into(),
            shape: shape.into(),
        }],
    };
    let context =
        authored_svg_context(&source, &generated, true).map_err(|error| error.to_string())?;
    assert_eq!(context.clips.ids.len(), 2);
    assert!(context.clips.ids.contains("authored"));
    assert!(context.clips.ids.contains("generated"));
    assert_eq!(context.clips.generated_ids.len(), 1);
    assert!(context.clips.generated_ids.contains("generated"));
    assert!(context.clips.has_object_bounding_box);
    assert!(context.text.matches_text(LABEL));
    Ok(())
}

fn context_matches(source: &str, text: &str) -> Result<bool, String> {
    let document = Document::parse(source).map_err(|error| error.to_string())?;
    Ok(collect(&document).matches_text(text))
}

#[test]
fn blockless_keyframes_rejects_authored_text_context() -> Result<(), String> {
    let valid = r#"<svg><style>@keyframes motion { from { opacity: 1; } }</style><text><tspan dy="0">Runtime</tspan></text></svg>"#;
    let blockless =
        r#"<svg><style>@keyframes motion;</style><text><tspan dy="0">Runtime</tspan></text></svg>"#;
    assert!(context_matches(valid, LABEL)?);
    assert!(!context_matches(blockless, LABEL)?);
    Ok(())
}

#[test]
fn malformed_keyframes_preludes_cannot_prove_authored_text_layout() -> Result<(), String> {
    for (prelude, expected) in [("motion", true), ("123", false), ("motion extra", false)] {
        let source = format!(
            r#"<svg><style>@keyframes {prelude} {{ from {{ opacity: 1; }} }}</style><text><tspan dy="0">{LABEL}</tspan></text></svg>"#
        );
        assert_eq!(context_matches(&source, LABEL)?, expected, "{prelude}");
    }
    Ok(())
}

#[test]
fn empty_keyframe_selector_cannot_prove_authored_text_layout() -> Result<(), String> {
    let valid = r#"<svg><style>@keyframes motion { from { opacity: 1; } }</style><text><tspan dy="0">Runtime</tspan></text></svg>"#;
    let empty = r#"<svg><style>@keyframes motion { { opacity: 1; } }</style><text><tspan dy="0">Runtime</tspan></text></svg>"#;
    assert!(context_matches(valid, LABEL)?);
    assert!(!context_matches(empty, LABEL)?);
    Ok(())
}

#[test]
fn nested_custom_property_tokens_in_keyframes_do_not_disqualify_text() -> Result<(), String> {
    let source = r#"<svg><style>@keyframes motion { from { --paint: [var(--stroke)]; } }</style><text><tspan dy="0">Runtime</tspan></text></svg>"#;
    assert!(context_matches(source, LABEL)?);
    Ok(())
}
