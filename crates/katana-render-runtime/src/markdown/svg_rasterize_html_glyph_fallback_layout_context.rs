use resvg::usvg;

pub(super) fn chunk_context(
    text: &usvg::Text,
    source: &super::sources::Chunk<'_>,
    prepared: &[super::PreparedSpan],
    single_chunk: bool,
) -> Option<LayoutContext> {
    let start = start_paint_supported(source, prepared, single_chunk);
    LayoutContext::new(text, source.chunk, prepared.first()?.font_size, start)
}

fn start_paint_supported(
    source: &super::sources::Chunk<'_>,
    prepared: &[super::PreparedSpan],
    single_chunk: bool,
) -> bool {
    single_chunk
        && source.chunk.anchor() == usvg::TextAnchor::Start
        && source.source_spans.len() == 1
        && source.dy.iter().all(|value| *value == 0.0)
        && prepared.iter().all(|span| span.baseline_shift == 0.0)
}

pub(super) struct LayoutContext {
    pub(super) css_scale: f32,
    pub(super) paint_css_size: f32,
    pub(super) origin_x: f32,
    pub(super) cursor: f64,
    pub(super) start_paint: bool,
}

impl LayoutContext {
    pub(super) fn new(
        text: &usvg::Text,
        chunk: &usvg::TextChunk,
        font_size: f32,
        start_paint: bool,
    ) -> Option<Self> {
        let css_scale = super::super::anchor::uniform_css_scale(text.abs_transform())?;
        let paint_css_size = super::super::anchor::paint_size(font_size, css_scale, css_scale)?;
        let origin_x = chunk.x().unwrap_or(0.0) * css_scale;
        let origin_x = if start_paint {
            snap_origin(origin_x)?
        } else {
            origin_x
        };
        origin_x.is_finite().then_some(Self {
            css_scale,
            paint_css_size,
            origin_x,
            cursor: 0.0,
            start_paint,
        })
    }

    pub(super) fn position(&self, offset: f32, y: f32) -> Option<tiny_skia::Point> {
        let advance = (self.cursor + f64::from(offset)) as f32;
        /* WHY: 描画原点だけを量子化し、各字形のadvanceはshaping精度を保つ。 */
        let x = self.origin_x
            + if self.start_paint {
                advance
            } else {
                super::positions::advance_position(advance)?
            };
        let y = if self.start_paint { snap_origin(y)? } else { y };
        (x.is_finite() && y.is_finite()).then_some(tiny_skia::Point::from_xy(x, y))
    }
}

fn snap_origin(value: f32) -> Option<f32> {
    super::super::anchor::trunc_layout_unit(value)
}

#[cfg(test)]
mod tests {
    use super::{LayoutContext, snap_origin};

    #[test]
    fn start_paint_keeps_fractional_advance_after_snapped_origin() -> Result<(), String> {
        let origin_x = snap_origin(2.01).ok_or("missing origin")?;
        assert_eq!(origin_x, 2.0);
        let mut context = LayoutContext {
            css_scale: 1.0,
            paint_css_size: 1.0,
            origin_x,
            cursor: 0.0,
            start_paint: true,
        };
        assert_eq!(
            context.position(0.0, 3.01).ok_or("missing position")?,
            tiny_skia::Point::from_xy(2.0, 3.0)
        );
        context.cursor += f64::from(0.1_f32);
        assert_eq!(
            context.position(0.0, 3.01).ok_or("missing position")?,
            tiny_skia::Point::from_xy(2.1, 3.0)
        );
        Ok(())
    }

    #[test]
    fn legacy_position_retains_per_glyph_quantization() -> Result<(), String> {
        let context = LayoutContext {
            css_scale: 1.0,
            paint_css_size: 1.0,
            origin_x: 2.01,
            cursor: 0.1,
            start_paint: false,
        };
        assert_eq!(
            context.position(0.0, 3.01).ok_or("missing position")?,
            tiny_skia::Point::from_xy(2.01 + 0.09375, 3.01)
        );
        Ok(())
    }
}
