use super::PreparedSpan;
use resvg::usvg;

pub(super) fn single_horizontal_alignment(
    text: &usvg::Text,
    prepared: &[PreparedSpan],
) -> Option<f32> {
    let [span] = prepared else { return None };
    let [chunk] = text.chunks() else { return None };
    let [source_span] = chunk.spans() else {
        return None;
    };
    if !alignment_text_supported(text, chunk, source_span, span) {
        return None;
    }
    if !horizontal_glyphs(&span.glyphs) {
        return None;
    }
    let anchor_x = chunk.x().unwrap_or(0.0);
    anchor_x.is_finite().then_some(anchor_x)
}

pub(super) fn alignment_text_supported(
    text: &usvg::Text,
    chunk: &usvg::TextChunk,
    source_span: &usvg::TextSpan,
    prepared: &PreparedSpan,
) -> bool {
    text.layouted().len() == 1
        && alignment_positioning_supported(text, chunk)
        && alignment_source_span_supported(source_span)
        && !prepared.glyphs.is_empty()
}

pub(super) fn alignment_positioning_supported(text: &usvg::Text, chunk: &usvg::TextChunk) -> bool {
    alignment_positioning_supported_with_dy(text, chunk, text.dy())
}

pub(in super::super) fn alignment_positioning_supported_with_dy(
    text: &usvg::Text,
    chunk: &usvg::TextChunk,
    dy: &[f32],
) -> bool {
    all_zero(text.dx())
        && alignment_dy_supported(dy)
        && all_zero(text.rotate())
        && text.writing_mode() == usvg::WritingMode::LeftToRight
        && matches!(chunk.text_flow(), usvg::TextFlow::Linear)
}

pub(super) fn alignment_source_span_supported(source_span: &usvg::TextSpan) -> bool {
    source_span.letter_spacing() == 0.0
        && source_span.word_spacing() == 0.0
        && source_span.text_length().is_none()
}

pub(super) fn alignment_dy_supported(values: &[f32]) -> bool {
    values.first().is_none_or(|first| first.is_finite())
        && values.iter().skip(1).all(|value| *value == 0.0)
}

pub(super) fn all_zero(values: &[f32]) -> bool {
    values.iter().all(|value| *value == 0.0)
}

pub(super) fn horizontal_glyphs(glyphs: &[usvg::layout::PositionedGlyph]) -> bool {
    glyphs.iter().all(|glyph| {
        let transform = glyph.transform();
        transform.sx.is_finite()
            && transform.sy.is_finite()
            && transform.sx > 0.0
            && transform.sy > 0.0
            && transform.sx == transform.sy
            && transform.kx == 0.0
            && transform.ky == 0.0
    })
}
