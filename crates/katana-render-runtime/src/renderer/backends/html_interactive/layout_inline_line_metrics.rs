use super::super::style::CssStyle;
use crate::markdown::svg_rasterize::SvgRasterizeOps;

#[derive(Clone, Copy, Default)]
pub(super) struct InlineLineMetrics {
    baseline: Option<f32>,
    descent: f32,
}

impl InlineLineMetrics {
    pub(super) fn add_box(&mut self, baseline: f32, height: f32) {
        self.baseline = Some(
            self.baseline
                .map_or(baseline, |current| current.max(baseline)),
        );
        self.descent = self.descent.max(height - baseline);
    }
    pub(super) fn baseline(&self) -> Option<f32> {
        self.baseline
    }

    pub(super) fn height(&self) -> Option<f32> {
        self.baseline.map(|ascent| ascent + self.descent)
    }

    pub(super) fn add_text(&mut self, text: &str, style: &CssStyle) {
        if text
            .chars()
            .all(super::super::layout_text::is_css_whitespace)
        {
            return;
        }
        if let Some(baseline) = SvgRasterizeOps::html_text_baseline_for_text(
            text,
            &style.font_family,
            style.font_size,
            style.font_weight,
            style.italic,
            style.line_height,
            style.normal_line_height,
        ) {
            self.baseline = Some(
                self.baseline
                    .map_or(baseline, |current| current.max(baseline)),
            );
            self.descent = self.descent.max(style.line_height - baseline);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, InlineLineMetrics};

    #[test]
    fn zero_sized_text_does_not_contribute_line_extents() {
        let mut style = CssStyle::browser_default();
        style.font_size = 0.0;
        style.line_height = 0.0;
        let mut metrics = InlineLineMetrics::default();
        metrics.add_text("visible source with zero font size", &style);
        assert_eq!(metrics.baseline(), None);
        assert_eq!(metrics.height(), None);
        metrics.add_text("visible text", &CssStyle::browser_default());
        assert!(metrics.height().is_some_and(|height| height > 0.0));
    }
}
