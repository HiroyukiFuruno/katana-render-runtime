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
fn css_styled_and_inline_authored_text_render_identically() -> Result<(), String> {
    let document = Document::parse(RENDERED_AUTHORED_SVG).map_err(|error| error.to_string())?;
    assert!(collect(&document).matches_text(LABEL));
    let render = |svg| {
        crate::markdown::svg_rasterize::SvgRasterizeOps::rasterize_html_svg(svg, 1.0)
            .map_err(|error| error.to_string())
    };
    let css_render = render(RENDERED_AUTHORED_SVG)?;
    let inline_render = render(INLINE_AUTHORED_SVG)?;
    assert_eq!((css_render.width, css_render.height), AUTHORED_FRAME_SIZE);
    assert_eq!(
        (inline_render.width, inline_render.height),
        AUTHORED_FRAME_SIZE
    );
    assert_eq!(css_render.rgba, inline_render.rgba);
    assert!(css_render.rgba.chunks_exact(4).any(|pixel| pixel[3] != 0));
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
