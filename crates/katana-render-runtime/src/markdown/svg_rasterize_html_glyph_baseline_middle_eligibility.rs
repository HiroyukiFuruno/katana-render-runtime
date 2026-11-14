use super::super::super::{alignment_dy_supported, all_zero, anchor, horizontal_glyphs};
use resvg::usvg::{self, TextFlow};

pub(in crate::markdown::svg_rasterize::html_glyph::baseline) fn layout_supported(
    text: &usvg::Text,
) -> bool {
    layout_supported_with_zero_first_dy(text, false)
}

pub(in crate::markdown::svg_rasterize::html_glyph::baseline) fn nested_layout_supported(
    text: &usvg::Text,
) -> bool {
    layout_supported_with_zero_first_dy(text, true)
        && text.dy().first().is_some_and(|first_dy| *first_dy == 0.0)
}

pub(super) fn layout_supported_with_zero_first_dy(
    text: &usvg::Text,
    allow_zero_first_dy: bool,
) -> bool {
    let [chunk] = text.chunks() else { return false };
    let dy = text.dy();
    /* WHY: usvgは最初の文字の座標とspanを伴う場合だけchunkを作る。 */
    let first_dy = dy[0];
    let first_span = &chunk.spans()[0];
    text.writing_mode() == usvg::WritingMode::LeftToRight
        && matches!(chunk.text_flow(), TextFlow::Linear)
        && alignment_dy_supported(dy)
        && (first_dy != 0.0 || allow_zero_first_dy)
        && all_zero(text.dx())
        && all_zero(text.rotate())
        && chunk.spans().iter().all(source_span_has_default_spacing)
        && supported_layout_first_glyph(text.layouted(), first_span.font_size().get()).is_some()
}

fn supported_layout_first_glyph(
    spans: &[usvg::layout::Span],
    font_size: f32,
) -> Option<&usvg::layout::PositionedGlyph> {
    let first = spans
        .iter()
        .find_map(|span| span.positioned_glyphs.first())?;
    spans
        .iter()
        .all(|span| {
            let glyphs = &span.positioned_glyphs;
            horizontal_glyphs(glyphs) && glyphs.iter().all(|glyph| glyph.font_size() == font_size)
        })
        .then_some(first)
}

fn source_span_has_default_spacing(span: &usvg::TextSpan) -> bool {
    span.letter_spacing() == 0.0 && span.word_spacing() == 0.0 && span.text_length().is_none()
}

pub(in crate::markdown::svg_rasterize::html_glyph::baseline) fn additional_middle_layout_supported(
    text: &usvg::Text,
) -> bool {
    let [first_chunk, _, ..] = text.chunks() else {
        return false;
    };
    let first = &first_chunk.spans()[0];
    /* WHY: 従来の単一行経路は保ち、明示複数行だけを同じ主font補正へ渡す。 */
    anchor::uniform_css_scale(text.abs_transform()).is_some()
        && text.writing_mode() == usvg::WritingMode::LeftToRight
        && all_zero(text.dx())
        && all_zero(text.rotate())
        && additional_sources_supported(text, first)
        && supported_layout_first_glyph(text.layouted(), first.font_size().get())
            .is_some_and(|glyph| same_layout_face(text.layouted(), glyph))
        && chunk_relative_positions_supported(text)
}

fn additional_sources_supported(text: &usvg::Text, first: &usvg::TextSpan) -> bool {
    text.chunks().iter().all(|chunk| {
        matches!(chunk.text_flow(), TextFlow::Linear)
            && chunk
                .spans()
                .iter()
                .all(|span| additional_span_supported(span, first))
    })
}

fn additional_span_supported(span: &usvg::TextSpan, first: &usvg::TextSpan) -> bool {
    span.font() == first.font()
        && span.font_size() == first.font_size()
        && span.dominant_baseline() == usvg::DominantBaseline::Middle
        && matches!(
            span.alignment_baseline(),
            usvg::AlignmentBaseline::Auto | usvg::AlignmentBaseline::Baseline
        )
        && source_span_has_default_spacing(span)
        && span.baseline_shift().is_empty()
        && no_decoration(span)
}

fn no_decoration(span: &usvg::TextSpan) -> bool {
    let decoration = span.decoration();
    decoration.underline().is_none()
        && decoration.overline().is_none()
        && decoration.line_through().is_none()
}

fn same_layout_face(spans: &[usvg::layout::Span], first: &usvg::layout::PositionedGlyph) -> bool {
    spans
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .skip(1)
        .all(|glyph| glyph.font == first.font)
}

fn chunk_relative_positions_supported(text: &usvg::Text) -> bool {
    if !text.dy().first().is_none_or(|value| *value == 0.0)
        || !text.dy().iter().all(|value| value.is_finite())
    {
        return false;
    }
    let mut offset = 0;
    for (index, chunk) in text.chunks().iter().enumerate() {
        let count = chunk.text().chars().count();
        if count == 0 || (index > 0 && !chunk.x().is_some_and(f32::is_finite)) {
            return false;
        }
        if text
            .dy()
            .iter()
            .skip(offset + 1)
            .take(count - 1)
            .any(|value| *value != 0.0)
        {
            return false;
        }
        offset += count;
    }
    text.dy().iter().skip(offset).all(|value| *value == 0.0)
}
