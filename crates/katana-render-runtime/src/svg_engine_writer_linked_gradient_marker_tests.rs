use super::super::{ALPHA_CHANNEL, RGBA_CHANNELS, parse_writer_svg, render_tree, usvg};

const MARKER_SVG_TEMPLATE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="56" height="32">
<defs>
{gradient_definition}
  <marker id="painted-marker" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" refX="10" refY="5" overflow="visible">
    <path d="M0 0 L10 5 L0 10 Z" fill="url(#marker-paint)"/>
  </marker>
</defs>
<path d="M5 16 L42 16" fill="none" stroke="#222222" marker-end="url(#painted-marker)"/>
</svg>"##;

const LINKED_GRADIENT: &str = r##"<linearGradient id="base-paint" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="10" y2="0">
  <stop offset="0" stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/>
</linearGradient>
<linearGradient id="marker-paint" href="#base-paint"/>"##;
const EXPLICIT_GRADIENT: &str = r##"<linearGradient id="marker-paint" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="10" y2="0">
  <stop offset="0" stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/>
</linearGradient>"##;

#[test]
fn linked_gradient_in_marker_matches_explicit_paint_through_writer_roundtrip() -> Result<(), String>
{
    let linked_svg = marker_svg(LINKED_GRADIENT);
    let explicit_svg = marker_svg(EXPLICIT_GRADIENT);
    let original = parse_writer_svg(&linked_svg, "linked-marker-gradient")?;
    let serialized = original.to_string(&usvg::WriteOptions::default());
    let roundtrip = parse_writer_svg(&serialized, "linked-marker-gradient-roundtrip")?;
    let explicit = render_svg(&explicit_svg, "explicit-marker-gradient")?;
    let solid_source = linked_svg.replace("url(#marker-paint)", "#ff00ff");
    let solid = render_svg(&solid_source, "solid-marker-control")?;
    let actual = render_tree(&original)?;
    let reparsed = render_tree(&roundtrip)?;
    assert!(
        has_visible_color(actual.data()),
        "marker was not rendered in color"
    );
    assert_eq!(actual.data(), explicit.data(), "linked gradient paint");
    assert_eq!(actual.data(), reparsed.data(), "writer roundtrip");
    assert_ne!(actual.data(), solid.data(), "marker gradient was ignored");
    Ok(())
}

fn marker_svg(gradient_definition: &str) -> String {
    MARKER_SVG_TEMPLATE.replace("{gradient_definition}", gradient_definition)
}

fn has_visible_color(pixels: &[u8]) -> bool {
    pixels
        .chunks_exact(RGBA_CHANNELS)
        .any(|rgba| rgba[ALPHA_CHANNEL] > 0 && (rgba[0] != rgba[1] || rgba[1] != rgba[2]))
}

fn render_svg(svg: &str, name: &str) -> Result<tiny_skia::Pixmap, String> {
    let tree = parse_writer_svg(svg, name)?;
    render_tree(&tree)
}
