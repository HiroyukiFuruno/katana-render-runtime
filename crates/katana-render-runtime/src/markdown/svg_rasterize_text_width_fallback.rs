use super::super::font::html_rasterizer_options;
use super::SvgRasterizeError;
use resvg::usvg;

const FALLBACK_SVG_WIDTH: u32 = 16_384;
const FALLBACK_HEIGHT_FACTOR: f32 = 4.0;
const FALLBACK_BASELINE_FACTOR: f32 = 2.0;

pub(super) fn fallback_text_width(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    letter_spacing: f32,
) -> Result<f32, SvgRasterizeError> {
    let style = if italic { "italic" } else { "normal" };
    let height = (font_size * FALLBACK_HEIGHT_FACTOR).max(1.0);
    let baseline = font_size * FALLBACK_BASELINE_FACTOR;
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{FALLBACK_SVG_WIDTH}" height="{height}"><text id="krr-text-measure" x="0" y="{baseline}" font-family="{}" font-size="{font_size}" font-weight="{font_weight}" font-style="{style}" letter-spacing="{letter_spacing}">{}</text></svg>"#,
        escape_xml_attribute(&canonical_svg_font_family(font_family)),
        escape_xml_text(text)
    );
    let options = html_rasterizer_options(&svg);
    let tree = super::super::text_shaping::with_validated_tree_parse(|| {
        usvg::Tree::from_str(&svg, &options)
    })
    .map_err(|error| SvgRasterizeError::ParseFailed(error.to_string()))?;
    Ok(tree
        .node_by_id("krr-text-measure")
        .map(|node| node.abs_bounding_box().width())
        .unwrap_or(0.0))
}

pub(super) fn canonical_svg_font_family(value: &str) -> String {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        value
            .split(',')
            .map(str::trim)
            .map(|family| match family.to_ascii_lowercase().as_str() {
                "system-ui" | "-apple-system" | "blinkmacsystemfont" => "System Font",
                _ => family,
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        value.to_string()
    }
}

pub(super) fn escape_xml_attribute(value: &str) -> String {
    escape_xml_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(super) fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
