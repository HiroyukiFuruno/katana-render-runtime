const HTML_FONT_SIZE_PRECISION: f32 = 100.0;
use super::features::TextSpacingTrim;

pub(in super::super) fn effective_html_font_size(size: f32) -> f32 {
    /* WHY: BlinkのFontCacheKeyと同じ小数2桁の切り下げをshapingと描画で共有する。 */
    (size * HTML_FONT_SIZE_PRECISION).floor() / HTML_FONT_SIZE_PRECISION
}

#[derive(Clone, Copy)]
pub(in crate::markdown::svg_rasterize) struct FontShape {
    pub(in crate::markdown::svg_rasterize) size: f32,
    pub(in crate::markdown::svg_rasterize) weight: u16,
    pub(in crate::markdown::svg_rasterize) italic: bool,
    pub(in crate::markdown::svg_rasterize) trim: TextSpacingTrim,
}

impl FontShape {
    pub(in crate::markdown::svg_rasterize) fn new(size: f32, weight: u16, italic: bool) -> Self {
        Self {
            size: effective_html_font_size(size),
            weight,
            italic,
            trim: TextSpacingTrim::Normal,
        }
    }

    pub(in crate::markdown::svg_rasterize) fn with_trim(mut self, trim: TextSpacingTrim) -> Self {
        self.trim = trim;
        self
    }
}
