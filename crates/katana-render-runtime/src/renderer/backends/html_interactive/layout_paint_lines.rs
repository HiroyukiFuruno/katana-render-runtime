use super::constants::FONT_WEIGHT_NORMAL;
use super::layout::HtmlLayoutRenderer;
use super::style::{CssStyle, CssTextAlign, CssTextSpacingTrim};
use super::svg::escape_xml;
use super::text_metrics::text_width;
use crate::markdown::svg_rasterize::{SvgRasterizeOps, TextSpacingTrim};

impl HtmlLayoutRenderer {
    pub(super) fn paint_text_lines(
        &mut self,
        lines: &[String],
        x: f32,
        width: f32,
        baseline_y: f32,
        style: &CssStyle,
    ) {
        for (index, line) in lines.iter().enumerate() {
            /* WHY: 空行は測定やSVG provenanceを作らず、enumerateのindexで行位置は保つ。 */
            if line.is_empty() {
                continue;
            }
            let measured_width = text_width(line, style);
            let offset = text_baseline_offset(line, style);
            self.paint_text_line(
                line,
                aligned_text_x_with_width(x, width, style, measured_width),
                baseline_y - style.font_size + offset + index as f32 * style.line_height,
                style,
                measured_width,
            );
        }
    }

    fn paint_text_line(
        &mut self,
        line: &str,
        x: f32,
        baseline_y: f32,
        style: &CssStyle,
        measured_width: f32,
    ) {
        let x = x - self.scroll_x;
        let y = baseline_y - self.scroll_y;
        let markup = self.text_line_markup(line, x, y, style, measured_width);
        let markup = self.generated_html_text_provenance.record(markup);
        self.svg.push_str(&markup);
    }

    fn text_line_markup(
        &mut self,
        line: &str,
        x: f32,
        y: f32,
        style: &CssStyle,
        measured_width: f32,
    ) -> String {
        let transformed = style.transformed_text(line);
        let feature_dx = font_feature_dx(&transformed, style);
        let text_length = measured_length_markup(feature_dx.as_deref(), measured_width);
        let (font_markup, content) = self
            .font_markup_cache
            .font_markup_at(&transformed, style, x);
        format!(
            r#"<text x="{x}" y="{y}" {} font-size="{}" fill="{}"{}{}{}{}{}{}{}>{}</text>"#,
            font_markup,
            SvgRasterizeOps::effective_html_font_size(style.font_size),
            escape_xml(&style.color),
            font_weight(style),
            font_style(style),
            text_decoration(style),
            letter_spacing(style),
            font_feature_settings(style),
            feature_dx.as_deref().unwrap_or_default(),
            text_length,
            content,
        )
    }
}

fn measured_length_markup(feature_dx: Option<&str>, width: f32) -> String {
    feature_dx.map_or_else(|| text_length(width), |_| String::new())
}

fn text_baseline_offset(line: &str, style: &CssStyle) -> f32 {
    style.text_baseline_offset.unwrap_or_else(|| {
        SvgRasterizeOps::html_text_baseline_for_text(
            line,
            &style.font_family,
            style.font_size,
            style.font_weight,
            style.italic,
            style.line_height,
            style.normal_line_height,
        )
        .unwrap_or(style.font_size)
    })
}

#[cfg(test)]
fn aligned_text_x(line: &str, x: f32, width: f32, style: &CssStyle) -> f32 {
    aligned_text_x_with_width(x, width, style, text_width(line, style))
}

fn aligned_text_x_with_width(x: f32, width: f32, style: &CssStyle, measured_width: f32) -> f32 {
    match style.text_align {
        CssTextAlign::Start => x,
        CssTextAlign::Center => x + (width - measured_width).max(0.0) / 2.0,
        CssTextAlign::End => x + (width - measured_width).max(0.0),
    }
}

fn text_length(measured_width: f32) -> String {
    format!(
        " textLength=\"{}\" lengthAdjust=\"spacingAndGlyphs\"",
        measured_width
    )
}

fn font_weight(style: &CssStyle) -> String {
    if style.font_weight == FONT_WEIGHT_NORMAL {
        String::new()
    } else {
        format!(" font-weight=\"{}\"", style.font_weight)
    }
}

fn font_style(style: &CssStyle) -> &'static str {
    if style.italic {
        " font-style=\"italic\""
    } else {
        ""
    }
}

fn text_decoration(style: &CssStyle) -> &'static str {
    if style.underline {
        " text-decoration=\"underline\""
    } else {
        ""
    }
}

fn letter_spacing(style: &CssStyle) -> String {
    if style.letter_spacing == 0.0 {
        String::new()
    } else {
        format!(" letter-spacing=\"{}\"", style.letter_spacing)
    }
}

fn font_feature_settings(style: &CssStyle) -> String {
    let Some(settings) = style.font_feature_settings.as_ref() else {
        return String::new();
    };
    format!(" font-feature-settings=\"{}\"", escape_xml(settings))
}

fn font_feature_dx(text: &str, style: &CssStyle) -> Option<String> {
    let dx = SvgRasterizeOps::shape_html_text_dx_with_trim(
        text,
        &style.font_family,
        style.font_size,
        style.font_weight,
        style.italic,
        style.font_feature_settings.as_deref(),
        match style.text_spacing_trim {
            CssTextSpacingTrim::Normal => TextSpacingTrim::Normal,
            CssTextSpacingTrim::SpaceAll => TextSpacingTrim::SpaceAll,
        },
    )?;
    Some(format!(
        " dx=\"{}\"",
        dx.iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    ))
}

#[cfg(test)]
mod tests {
    use super::super::{
        layout::HtmlLayoutRenderer,
        style::{CssStyle, CssTextAlign},
        text_metrics::text_width,
    };
    use super::aligned_text_x;
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use crate::renderer::backends::html_interactive::tests::first_text_x;
    use std::collections::HashMap;

    #[test]
    fn text_alignment_and_typography_attributes_are_written_to_svg() {
        let viewport = HtmlBrowserViewport {
            width: 400,
            height: 300,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let lines = vec!["（JSC 案件）".to_string(), "x".to_string()];

        let end_style = typography_test_style();
        renderer.paint_text_lines(&lines, 10.0, 120.0, 24.0, &end_style);

        let expected_x = aligned_text_x("（JSC 案件）", 10.0, 120.0, &end_style);
        let text_attr = first_text_x(&renderer.svg);

        assert_eq!(text_attr, Some(expected_x));
        assert!(renderer.svg.contains(" font-weight=\"700\""));
        assert!(renderer.svg.contains(" font-style=\"italic\""));
        assert!(renderer.svg.contains(" text-decoration=\"underline\""));
        assert!(renderer.svg.contains(" dx=\""), "{}", renderer.svg);
        assert!(renderer.svg.contains(" letter-spacing=\"1\""));
        assert!(
            renderer
                .svg
                .contains(" font-feature-settings=\"&quot;palt&quot; 1\"")
        );
    }

    fn typography_test_style() -> CssStyle {
        let mut style = CssStyle::browser_default();
        style.text_align = CssTextAlign::End;
        style.letter_spacing = 1.0;
        style.font_weight = 700;
        style.italic = true;
        style.underline = true;
        style.font_family =
            "\"Noto Sans JP\", \"Hiragino Sans\", \"Yu Gothic\", sans-serif".to_string();
        style.font_feature_settings = Some(r#""palt" 1"#.to_string());
        style
    }

    #[test]
    fn alignment_center_uses_width_offset_and_start_keeps_origin() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let mut style = CssStyle::browser_default();
        style.text_align = CssTextAlign::Center;
        let width = 100.0;
        let expected = aligned_text_x("ab", 5.0, width, &style);
        let text_width = text_width("ab", &style);
        assert_eq!(expected, 5.0 + (width - text_width).max(0.0_f32) / 2.0);
        renderer.paint_text_lines(&["ab".to_string()], 5.0, width, 24.0, &style);
        assert!(renderer.svg.contains("<text"));

        let mut start_style = CssStyle::browser_default();
        start_style.text_align = CssTextAlign::Start;
        let expected = aligned_text_x("ab", 5.0, width, &start_style);
        assert_eq!(expected, 5.0);
    }

    #[test]
    fn font_style_helpers_emit_expected_markers() {
        let viewport = HtmlBrowserViewport {
            width: 10,
            height: 180,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 2.0, &HashMap::new(), None);
        let style = CssStyle::browser_default();
        renderer.paint_text_line("hello", 2.0, 12.0, &style, text_width("hello", &style));
        assert!(renderer.svg.contains(" y=\"10\""));
        assert!(renderer.svg.contains(" textLength=\""));
    }
}

#[cfg(test)]
#[path = "layout_paint_lines_empty_tests.rs"]
mod empty_text_tests;
