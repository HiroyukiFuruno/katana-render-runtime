use super::{SvgRasterizeOps, TextSpacingTrim, text_shaping};

#[derive(Clone, Copy)]
pub(crate) struct HtmlFontStyle<'a> {
    pub(crate) family: &'a str,
    pub(crate) size: f32,
    pub(crate) weight: u16,
    pub(crate) italic: bool,
    pub(crate) language: Option<&'a str>,
}

pub(crate) struct HtmlPaintFontRun {
    pub(crate) family: String,
    pub(crate) text: String,
    pub(crate) variations: Vec<resvg::usvg::FontVariation>,
}

impl HtmlPaintFontRun {
    pub(crate) fn plain(family: String, text: String) -> Self {
        Self {
            family,
            text,
            variations: Vec::new(),
        }
    }
}

impl SvgRasterizeOps {
    pub(crate) fn html_text_paint_runs(
        text: &str,
        style: HtmlFontStyle<'_>,
    ) -> Option<Vec<HtmlPaintFontRun>> {
        #[cfg(target_os = "macos")]
        if let Some(plan) = text_shaping::NativeHtmlRunPlan::build(text, style) {
            return plan.paint_runs();
        }
        Self::html_text_font_runs(text, style.family, style.weight, style.italic).map(|runs| {
            runs.into_iter()
                .map(|(family, text)| HtmlPaintFontRun::plain(family, text))
                .collect()
        })
    }

    pub(crate) fn native_html_text_advance(
        text: &str,
        style: HtmlFontStyle<'_>,
        letter_spacing: f32,
        features: Option<&str>,
        trim: TextSpacingTrim,
    ) -> Option<f32> {
        #[cfg(target_os = "macos")]
        {
            let plan = text_shaping::NativeHtmlRunPlan::build(text, style)?;
            let shape = text_shaping::FontShape::new(style.size, style.weight, style.italic)
                .with_trim(trim);
            let spacing = text.chars().count().saturating_sub(1) as f32 * letter_spacing;
            plan.advance(shape, features)
                .map(|advance| advance + spacing)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (
                text,
                style.size,
                style.language,
                letter_spacing,
                features,
                trim,
            );
            None
        }
    }

    pub(crate) fn html_control_arial_family() -> &'static str {
        use super::font::platform::{PlatformFontStrategy, font_platform};

        font_platform().control_arial_family()
    }

    pub(crate) fn localized_html_sans_serif(language: &str) -> Option<&'static str> {
        use super::font::platform::{PlatformFontStrategy, font_platform};

        font_platform().localized_sans_serif(language)
    }

    pub(crate) fn localized_html_font_family(family: &str, sans_serif: &str) -> Option<String> {
        super::font::platform::localized_font_family(family, sans_serif)
    }

    pub(crate) fn html_text_baseline_for_text(
        text: &str,
        family: &str,
        size: f32,
        weight: u16,
        italic: bool,
        line_height: f32,
        normal_line_height: bool,
    ) -> Option<f32> {
        text_shaping::html_text_baseline_for_text(
            text,
            family,
            size,
            weight,
            italic,
            line_height,
            normal_line_height,
        )
    }
    pub(crate) fn html_line_metrics(
        text: &str,
        font_family: &str,
        font_size: f32,
        font_weight: u16,
        italic: bool,
    ) -> Option<(f32, f32)> {
        text_shaping::html_line_metrics(text, font_family, font_size, font_weight, italic)
    }
    pub(crate) fn shape_html_paint_dx(
        text: &str,
        style: HtmlFontStyle<'_>,
        font_feature_settings: Option<&str>,
        trim: TextSpacingTrim,
    ) -> Option<Vec<f32>> {
        #[cfg(target_os = "macos")]
        if let Some(plan) = text_shaping::NativeHtmlRunPlan::build(text, style) {
            let shape = text_shaping::FontShape::new(style.size, style.weight, style.italic)
                .with_trim(trim);
            return plan.dx(text.chars().count(), shape, font_feature_settings);
        }
        text_shaping::shaped_html_text_dx_with_trim(
            text,
            style.family,
            style.size,
            style.weight,
            style.italic,
            font_feature_settings,
            trim,
        )
    }

    pub(crate) fn effective_html_font_size(size: f32) -> f32 {
        text_shaping::effective_html_font_size(size)
    }

    pub(crate) fn html_input_character_metrics(
        family: &str,
        size: f32,
        weight: u16,
        italic: bool,
    ) -> Option<text_shaping::InputCharacterMetrics> {
        text_shaping::html_input_character_metrics(family, size, weight, italic)
    }

    pub(crate) fn html_text_font_runs(
        text: &str,
        font_family: &str,
        font_weight: u16,
        italic: bool,
    ) -> Option<Vec<(String, String)>> {
        text_shaping::html_text_font_runs(text, font_family, font_weight, italic)
    }
}
