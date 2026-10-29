use resvg::usvg;

pub(super) struct LayoutContext {
    pub(super) css_scale: f32,
    pub(super) paint_css_size: f32,
    pub(super) origin_x: f32,
    pub(super) cursor: f64,
}

impl LayoutContext {
    pub(super) fn new(text: &usvg::Text, chunk: &usvg::TextChunk, font_size: f32) -> Option<Self> {
        let css_scale = super::super::anchor::uniform_css_scale(text.abs_transform())?;
        let paint_css_size = super::super::anchor::paint_size(font_size, css_scale, css_scale)?;
        let origin_x = chunk.x().unwrap_or(0.0) * css_scale;
        origin_x.is_finite().then_some(Self {
            css_scale,
            paint_css_size,
            origin_x,
            cursor: 0.0,
        })
    }
}
