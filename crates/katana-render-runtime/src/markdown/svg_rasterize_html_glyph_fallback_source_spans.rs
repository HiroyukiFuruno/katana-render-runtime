use resvg::usvg;

pub(super) fn supported(
    chunk: &usvg::TextChunk,
    source_spans: &[usvg::TextSpan],
    layout_spans: &[usvg::layout::Span],
) -> bool {
    super::super::chunk::ranges_cover_chunk(chunk)
        && source_spans
            .iter()
            .zip(layout_spans)
            .all(|(source, layout)| source_layout_supported(chunk, source, layout))
        && same_baseline(layout_spans)
}

fn source_layout_supported(
    chunk: &usvg::TextChunk,
    source: &usvg::TextSpan,
    layout: &usvg::layout::Span,
) -> bool {
    let Some(content) = chunk.text().get(source.start()..source.end()) else {
        return false;
    };
    source_style_supported(source)
        && super::super::alignment_source_span_supported(source)
        && !layout.positioned_glyphs.is_empty()
        && super::super::anchor::nonempty_ordered_scalar_per_glyph(
            content,
            &layout.positioned_glyphs,
        )
}

fn source_style_supported(source: &usvg::TextSpan) -> bool {
    let decoration = source.decoration();
    source.apply_kerning()
        && !source.small_caps()
        && standard_font_variations(source.font())
        && source.baseline_shift().is_empty()
        && decoration.underline().is_none()
        && decoration.overline().is_none()
        && decoration.line_through().is_none()
}

fn same_baseline(layout_spans: &[usvg::layout::Span]) -> bool {
    let mut baseline = None;
    for glyph in layout_spans.iter().flat_map(|span| &span.positioned_glyphs) {
        let y = glyph.transform().ty;
        if !y.is_finite() || baseline.is_some_and(|baseline| baseline != y) {
            return false;
        }
        baseline = Some(y);
    }
    baseline.is_some()
}

fn standard_font_variations(font: &usvg::Font) -> bool {
    let [axis] = font.variations() else {
        return false;
    };
    axis.tag == *b"wght" && axis.value == f32::from(font.weight())
}
