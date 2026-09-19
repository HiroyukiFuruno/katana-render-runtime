use super::SvgRasterizeError;
use super::font::html_rasterizer_options;
use super::text_shaping::shaped_text_width;
use resvg::usvg;

const FALLBACK_SVG_WIDTH: u32 = 16_384;
const FALLBACK_HEIGHT_FACTOR: f32 = 4.0;
const FALLBACK_BASELINE_FACTOR: f32 = 2.0;

struct HtmlTextMeasure<'a> {
    font_family: &'a str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&'a str>,
}

pub(super) fn measure_html_text(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Result<f32, SvgRasterizeError> {
    HtmlTextMeasure {
        font_family,
        font_size,
        font_weight,
        italic,
        letter_spacing,
        font_feature_settings,
    }
    .measure(text)
}

impl HtmlTextMeasure<'_> {
    fn measure(&self, text: &str) -> Result<f32, SvgRasterizeError> {
        if text.is_empty() {
            return Ok(0.0);
        }
        shaped_text_width(
            text,
            self.font_family,
            self.font_size,
            self.font_weight,
            self.italic,
            self.letter_spacing,
            self.font_feature_settings,
        )
        .map_or_else(
            || {
                fallback_text_width(
                    text,
                    self.font_family,
                    self.font_size,
                    self.font_weight,
                    self.italic,
                    self.letter_spacing,
                )
            },
            Ok,
        )
    }
}

fn fallback_text_width(
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
        escape_xml_attribute(font_family),
        escape_xml_text(text)
    );
    let tree = usvg::Tree::from_str(&svg, &html_rasterizer_options(&svg))
        .map_err(|error| SvgRasterizeError::ParseFailed(error.to_string()))?;
    Ok(tree
        .node_by_id("krr-text-measure")
        .map(|node| node.abs_bounding_box().width())
        .unwrap_or(0.0))
}

fn escape_xml_attribute(value: &str) -> String {
    escape_xml_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::{fallback_text_width, measure_html_text};

    #[test]
    fn empty_html_text_has_zero_width() {
        let measured = measure_html_text("", "sans-serif", 16.0, 400, false, 0.0, None);

        assert!(matches!(measured, Ok(0.0)));
    }

    #[test]
    fn fallback_measurement_supports_bold_italic_text() {
        let measured = fallback_text_width("Bold", "sans-serif", 16.0, 700, true, 0.0);

        assert!(matches!(measured, Ok(width) if width > 0.0));
    }
}
