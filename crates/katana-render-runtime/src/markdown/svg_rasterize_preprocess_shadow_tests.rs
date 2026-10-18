use super::normalize_filter_shadow_colors;
use crate::markdown::svg_rasterize::SvgRasterizeOps;

const SVG_TEST_WIDTH: u32 = 64;
const SVG_TEST_HEIGHT: u32 = 48;
const SHADOW_SOURCE_X: u32 = 18;
const SHADOW_SOURCE_Y: u32 = 12;
const SHADOW_SOURCE_WIDTH: u32 = 16;
const SHADOW_SOURCE_HEIGHT: u32 = 16;
const SHADOW_SAMPLE_RIGHT_EDGE: u32 = SHADOW_SOURCE_X + SHADOW_SOURCE_WIDTH;
const SHADOW_SAMPLE_BOTTOM_EDGE: u32 = SHADOW_SOURCE_Y + SHADOW_SOURCE_HEIGHT;
const RGBA_CHANNEL_COUNT: usize = 4;
const RGB_CHANNEL_COUNT: usize = 3;

#[test]
fn svg_css_shadow_normalizes_only_filter_rgba_colors() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.shape { color: rgba(1,2,3,.4); filter: drop-shadow(1px 2px 2px rgba(185,185,185,0.7)) !important; --text: "rgba(1,2,3,.4)"; /* rgba(1,2,3,.4) */ } @media (min-width: 2px) { .other { filter: drop-shadow(0 1px 1px rgba(1,2,3,.4)); } }</style><rect style="color: rgba(1,2,3,.4); filter: drop-shadow(0 1px 1px rgba(4,5,6,.7)) !important"/><rect filter="drop-shadow(0 1px 1px rgba(7,8,9,.7))"/><rect style="filter: drop-shadow(0 1px 1px rgba(10&#44;11,12,.7))"/></svg>"#;
    let output = normalize_filter_shadow_colors(source);

    assert!(output.contains("filter: drop-shadow(1px 2px 2px #B9B9B9B3) !important"));
    assert!(output.contains("filter: drop-shadow(0 1px 1px #01020366)"));
    assert!(output.contains("color: rgba(1,2,3,.4)"));
    assert!(output.contains("--text: \"rgba(1,2,3,.4)\"; /* rgba(1,2,3,.4) */"));
    assert!(output.contains("filter=\"drop-shadow(0 1px 1px rgba(7,8,9,.7))\""));
    assert!(output.contains("rgba(10&#44;11,12,.7)"));
}

#[test]
fn svg_css_shadow_keeps_mermaid_style_declarations_while_rewriting_filter() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.edgePath .path { stroke: #333; stroke-width: 2px; color: #b9b9b9; --mermaid-shadow: url("shadow.svg"); filter: drop-shadow(1px 2px 2px rgba(185,185,185,0.7)) !important; } .fallback { filter: drop-shadow(1px 2px 2px var(--tone, rgba(185,185,185,0.7))); }</style></svg>"#;
    let output = normalize_filter_shadow_colors(source);

    assert!(output.contains("stroke: #333; stroke-width: 2px; color: #b9b9b9;"));
    assert!(output.contains("--mermaid-shadow: url(\"shadow.svg\");"));
    assert!(output.contains("filter: drop-shadow(1px 2px 2px #B9B9B9B3) !important"));
    assert!(output.contains("drop-shadow(1px 2px 2px var(--tone, rgba(185,185,185,0.7)))"));
}

#[test]
fn svg_css_shadow_rgba_hex_and_presentation_pixels_match() -> Result<(), String> {
    let rgba = shadow_image("filter: drop-shadow(1px 2px 2px rgba(185,185,185,0.7))")?;
    let hex = shadow_image("filter: drop-shadow(1px 2px 2px #B9B9B9B3)")?;
    let presentation = SvgRasterizeOps::rasterize_svg(
        &shadow_svg(
            "",
            " filter=\"drop-shadow(1px 2px 2px rgba(185,185,185,0.7))\"",
        ),
        1.0,
    )
    .map_err(|error| error.to_string())?;

    assert_eq!(rgba.rgba, hex.rgba);
    assert_eq!(rgba.rgba, presentation.rgba);
    assert!(has_shadow_pixels(&rgba.rgba, rgba.width));
    Ok(())
}

fn shadow_image(style: &str) -> Result<crate::markdown::svg_rasterize::RasterizedSvg, String> {
    SvgRasterizeOps::rasterize_svg(&shadow_svg(style, ""), 1.0).map_err(|error| error.to_string())
}

fn shadow_svg(style: &str, extra_attribute: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{SVG_TEST_WIDTH}\" height=\"{SVG_TEST_HEIGHT}\"><rect x=\"{SHADOW_SOURCE_X}\" y=\"{SHADOW_SOURCE_Y}\" width=\"{SHADOW_SOURCE_WIDTH}\" height=\"{SHADOW_SOURCE_HEIGHT}\" fill=\"#000\" style=\"{style}\"{extra_attribute}/></svg>"
    )
}

fn has_shadow_pixels(rgba: &[u8], width: u32) -> bool {
    rgba.chunks_exact(RGBA_CHANNEL_COUNT)
        .enumerate()
        .any(|(index, pixel)| {
            let x = index as u32 % width;
            let y = index as u32 / width;
            (x >= SHADOW_SAMPLE_RIGHT_EDGE || y >= SHADOW_SAMPLE_BOTTOM_EDGE)
                && pixel.get(RGB_CHANNEL_COUNT).is_some_and(|alpha| *alpha > 0)
                && pixel[..RGB_CHANNEL_COUNT]
                    .iter()
                    .any(|channel| *channel > 0)
        })
}
