use super::normalize_marker_selectors;
use crate::markdown::svg_rasterize::{RasterizedSvg, SvgRasterizeError, SvgRasterizeOps};

const SVG_WIDTH: u32 = 32;
const SVG_HEIGHT: u32 = 16;
const MAX_DOCUMENT_IDS: usize = 128;
const MAX_SELECTOR_MEMBERS: usize = 512;
const MAX_RULE_EDITS: usize = 256;
const MAX_SVG_BYTES: usize = 72 * 1024;
const MAX_STYLE_BYTES: usize = 48 * 1024;
const EXPANSION_PREFIX_REPETITIONS: usize = 700;
const RGBA_CHANNELS: usize = 4;
const RGB_CHANNELS: usize = 3;
const RED: [u8; RGB_CHANNELS] = [255, 0, 0];
const BLUE: [u8; RGB_CHANNELS] = [0, 0, 255];
const GREEN: [u8; RGB_CHANNELS] = [0, 255, 0];

#[test]
fn valid_id_and_selector_caps_preserve_source_and_public_control_pixels() {
    let id_capped = id_capped_svg();
    assert!(id_capped.len() < MAX_SVG_BYTES);
    assert_eq!(normalize_marker_selectors(id_capped.clone()), id_capped);
    assert_visible_color(&rendered_pixels(&id_capped), GREEN, "ID cap control");

    let selector_capped = selector_capped_svg();
    assert_eq!(
        normalize_marker_selectors(selector_capped.clone()),
        selector_capped
    );
}

#[test]
fn zero_match_sentinel_collision_keeps_public_pixels_unchanged() {
    let candidate = svg(
        "<style>[id$=\"missing\"]{fill:#ff0000!important}</style>",
        "<rect id=\"__krr_missing_marker_id_0__\" width=\"12\" height=\"16\" fill=\"#0000ff\"/>",
    );
    let control = svg(
        "<style>.unmatched{fill:#ff0000!important}</style>",
        "<rect id=\"__krr_missing_marker_id_0__\" width=\"12\" height=\"16\" fill=\"#0000ff\"/>",
    );
    let candidate_pixels = rendered_pixels(&candidate);
    let control_pixels = rendered_pixels(&control);
    assert_same_pixels(&candidate_pixels, &control_pixels);
    assert_visible_color(&candidate_pixels, BLUE, "zero-match candidate");
    assert_visible_color(&control_pixels, BLUE, "unmatched-class control");
}

#[test]
fn multiple_style_expansions_match_exact_id_public_pixels() {
    let suffix = svg(
        "<style>[id$=\"end\"]{fill:#ff0000!important}</style><style>[id$=\"start\"]{fill:#0000ff!important}</style>",
        colored_rectangles(),
    );
    let exact = svg(
        "<style>[id=\"actual-end\"],[id=\"other-end\"]{fill:#ff0000!important}</style><style>[id=\"actual-start\"]{fill:#0000ff!important}</style>",
        colored_rectangles(),
    );
    let suffix_pixels = rendered_pixels(&suffix);
    let exact_pixels = rendered_pixels(&exact);
    assert_same_pixels(&suffix_pixels, &exact_pixels);
    assert_visible_color(&suffix_pixels, RED, "suffix selectors");
    assert_visible_color(&suffix_pixels, BLUE, "suffix selectors");
}

#[test]
fn valid_unsupported_attribute_selector_preserves_bytes_and_control_pixels() {
    let source = svg(
        "<style>[class$=\"unused\"]{fill:#ff0000!important}</style>",
        "<rect class=\"control\" width=\"12\" height=\"16\" fill=\"#00ff00\"/>",
    );
    assert_eq!(normalize_marker_selectors(source.clone()), source);
    assert_visible_color(
        &rendered_pixels(&source),
        GREEN,
        "unsupported selector control",
    );
}

#[test]
fn valid_large_selector_expansion_budget_preserves_original_source() {
    let source = expansion_capped_svg();
    assert!(source.len() < MAX_SVG_BYTES);
    assert_eq!(normalize_marker_selectors(source.clone()), source);
}

#[test]
fn valid_style_and_rule_edit_caps_preserve_source_and_control_pixels() {
    for source in [many_style_nodes_svg(), many_style_rules_svg()] {
        assert!(source.len() < MAX_SVG_BYTES);
        assert_eq!(normalize_marker_selectors(source.clone()), source);
        assert_visible_color(&rendered_pixels(&source), GREEN, "edit cap control");
    }
}

fn id_capped_svg() -> String {
    let mut body = (0..MAX_DOCUMENT_IDS)
        .map(|index| format!("<rect id=\"decoy-{index}\"/>"))
        .collect::<String>();
    body.push_str("<rect id=\"visible-control\" width=\"12\" height=\"16\" fill=\"#00ff00\"/>");
    svg(
        "<style>[id$=\"end\"]{fill:#ff0000!important}</style>",
        &body,
    )
}

fn selector_capped_svg() -> String {
    let mut selectors = ".absent,".repeat(MAX_SELECTOR_MEMBERS);
    selectors.push_str("[id$=\"missing\"]");
    let style = format!("<style>{selectors}{{fill:#ff0000!important}}</style>");
    svg(
        &style,
        "<rect id=\"visible-control\" width=\"12\" height=\"16\" fill=\"#00ff00\"/>",
    )
}

fn expansion_capped_svg() -> String {
    let prefix = ".absent ".repeat(EXPANSION_PREFIX_REPETITIONS);
    let style = format!("<style>{prefix}[id$=\"end\"]{{fill:#ff0000}}</style>");
    let mut body = (0..MAX_DOCUMENT_IDS - 1)
        .map(|index| format!("<rect id=\"target-{index}-end\"/>"))
        .collect::<String>();
    body.push_str("<rect id=\"visible-control\" width=\"12\" height=\"16\" fill=\"#00ff00\"/>");
    assert!(style.len() < MAX_STYLE_BYTES);
    svg(&style, &body)
}

fn many_style_nodes_svg() -> String {
    let styles = "<style>[id$=\"end\"]{fill:red}</style>".repeat(MAX_RULE_EDITS + 1);
    svg(
        &styles,
        "<rect id=\"actual-end\"/><rect id=\"visible-control\" width=\"12\" height=\"16\" fill=\"#00ff00\"/>",
    )
}

fn many_style_rules_svg() -> String {
    let rules = "[id$=\"end\"]{fill:red}".repeat(MAX_RULE_EDITS + 1);
    let style = format!("<style>{rules}</style>");
    svg(
        &style,
        "<rect id=\"actual-end\"/><rect id=\"visible-control\" width=\"12\" height=\"16\" fill=\"#00ff00\"/>",
    )
}

fn colored_rectangles() -> &'static str {
    "<rect id=\"actual-end\" width=\"8\" height=\"16\"/><rect id=\"other-end\" x=\"8\" width=\"8\" height=\"16\"/><rect id=\"actual-start\" x=\"16\" width=\"8\" height=\"16\"/>"
}

fn svg(style: &str, body: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{SVG_WIDTH}\" height=\"{SVG_HEIGHT}\">{style}{body}</svg>"
    )
}

fn rendered_pixels(source: &str) -> Result<RasterizedSvg, SvgRasterizeError> {
    SvgRasterizeOps::rasterize_svg(source, 1.0)
}

fn assert_same_pixels(
    left: &Result<RasterizedSvg, SvgRasterizeError>,
    right: &Result<RasterizedSvg, SvgRasterizeError>,
) {
    assert!(
        left.as_ref().is_ok_and(|left| {
            right.as_ref().is_ok_and(|right| {
                left.width == right.width && left.height == right.height && left.rgba == right.rgba
            })
        }),
        "rasterization failed or dimensions/RGBA differed"
    );
}

fn assert_visible_color(
    pixels: &Result<RasterizedSvg, SvgRasterizeError>,
    color: [u8; RGB_CHANNELS],
    context: &str,
) {
    assert!(
        pixels.as_ref().is_ok_and(|image| {
            image.width == SVG_WIDTH
                && image.height == SVG_HEIGHT
                && image
                    .rgba
                    .chunks_exact(RGBA_CHANNELS)
                    .any(|pixel| pixel[..RGB_CHANNELS] == color && pixel[RGB_CHANNELS] > 0)
        }),
        "expected visible {color:?} for {context}"
    );
}
