const HTML_FONT_SIZE_PRECISION: f32 = 100.0;

pub(in super::super) fn effective_html_font_size(size: f32) -> f32 {
    /* WHY: BlinkのFontCacheKeyと同じ小数2桁の切り下げをshapingと描画で共有する。 */
    (size * HTML_FONT_SIZE_PRECISION).floor() / HTML_FONT_SIZE_PRECISION
}

#[derive(Clone, Copy)]
pub(super) struct FontShape {
    pub(super) size: f32,
    pub(super) weight: u16,
    pub(super) italic: bool,
}

impl FontShape {
    pub(super) fn new(size: f32, weight: u16, italic: bool) -> Self {
        Self {
            size: effective_html_font_size(size),
            weight,
            italic,
        }
    }
}
