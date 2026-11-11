use super::{PreparedSpan, XAlignment, anchor};
use resvg::usvg;

pub(super) fn apply(text: &usvg::Text, prepared: &mut [PreparedSpan]) -> bool {
    let Some(alignments) = alignments(text, prepared) else {
        return false;
    };
    for (span, alignment) in prepared.iter_mut().zip(alignments) {
        span.x_alignment = Some(alignment);
    }
    true
}

fn alignments(text: &usvg::Text, prepared: &[PreparedSpan]) -> Option<Vec<XAlignment>> {
    if !text_supported(text, prepared) {
        return None;
    }
    let css_scale = anchor::uniform_css_scale(text.abs_transform())?;
    let mut character_offset: usize = 0;
    let mut result = Vec::with_capacity(prepared.len());
    for ((chunk, span), layout) in text.chunks().iter().zip(prepared).zip(text.layouted()) {
        let character_count = chunk.text().chars().count();
        let end = character_offset.checked_add(character_count)?;
        let dy = text.dy().get(character_offset..end)?;
        if !super::alignment_dy_supported(dy) {
            return None;
        }
        result.push(run_alignment(chunk, span, layout, css_scale)?);
        character_offset = end;
    }
    (character_offset == text.dy().len()).then_some(result)
}

fn run_alignment(
    chunk: &usvg::TextChunk,
    span: &PreparedSpan,
    layout: &usvg::layout::Span,
    css_scale: f32,
) -> Option<XAlignment> {
    let anchor_x = chunk.x()?;
    if !anchor_x.is_finite() || !run_supported(chunk, span, layout) {
        return None;
    }
    Some(XAlignment {
        anchor_x,
        layout: None,
        origin_snap_css_scale: Some(css_scale),
    })
}

fn text_supported(text: &usvg::Text, prepared: &[PreparedSpan]) -> bool {
    text.chunks().len() > 1
        && text.chunks().len() == prepared.len()
        && text.layouted().len() == prepared.len()
        && text.writing_mode() == usvg::WritingMode::LeftToRight
        && super::all_zero(text.dx())
        && super::all_zero(text.rotate())
}

fn run_supported(
    chunk: &usvg::TextChunk,
    span: &PreparedSpan,
    layout: &usvg::layout::Span,
) -> bool {
    let [source] = chunk.spans() else {
        return false;
    };
    matches!(chunk.text_flow(), usvg::TextFlow::Linear)
        && source_supported(source, chunk.text())
        && super::horizontal_glyphs(&span.glyphs)
        /* WHY: run_alignmentは原点だけを補正するため、1 glyphの短いrunも扱える。 */
        && anchor::nonempty_ordered_scalar_per_glyph(chunk.text(), &span.glyphs)
        && constant_baseline(span)
        && layout_matches(layout, span)
        && span.x_alignment.is_none()
        && span.chunk_alignment.is_none()
}

fn source_supported(source: &usvg::TextSpan, content: &str) -> bool {
    let decoration = source.decoration();
    super::alignment_source_span_supported(source)
        && source.start() == 0
        && source.end() == content.len()
        && source.baseline_shift().is_empty()
        && !source.small_caps()
        && decoration.underline().is_none()
        && decoration.overline().is_none()
        && decoration.line_through().is_none()
}

fn constant_baseline(span: &PreparedSpan) -> bool {
    /* WHY: 前段で非空と有限座標を確認済み。各グリフの変換は一度だけ計算する。 */
    let mut baselines = span.glyphs.iter().map(|glyph| glyph.transform().ty);
    baselines
        .next()
        .is_none_or(|first| baselines.all(|baseline| baseline == first))
}

pub(super) fn layout_matches(layout: &usvg::layout::Span, span: &PreparedSpan) -> bool {
    layout.positioned_glyphs.len() == span.glyphs.len()
        && layout
            .positioned_glyphs
            .iter()
            .zip(&span.glyphs)
            .all(|(source, glyph)| {
                source.id == glyph.id
                    && source.font == glyph.font
                    && source.font_size() == glyph.font_size()
                    && source.text == glyph.text
                    && source.transform() == glyph.transform()
            })
}
