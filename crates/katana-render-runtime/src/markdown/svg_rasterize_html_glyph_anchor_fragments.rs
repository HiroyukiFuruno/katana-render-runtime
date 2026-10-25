use super::super::PreparedSpan;
use resvg::usvg;

pub(super) fn multi_fragment_anchor_x(text: &usvg::Text, prepared: &[PreparedSpan]) -> Option<f32> {
    let [chunk] = text.chunks() else { return None };
    if !alignment_supported(text, chunk, prepared) {
        return None;
    }
    let anchor_x = chunk.x().unwrap_or(0.0);
    anchor_x.is_finite().then_some(anchor_x)
}

fn alignment_supported(
    text: &usvg::Text,
    chunk: &usvg::TextChunk,
    prepared: &[PreparedSpan],
) -> bool {
    prepared.len() >= 2
        && chunk.spans().len() == prepared.len()
        && super::super::alignment_positioning_supported(text, chunk)
        && source_spans_supported(chunk)
        && prepared_spans_supported(prepared)
        && layout_fragments_match(text, prepared)
        && ordered_ltr_glyphs(chunk, prepared)
}

fn source_spans_supported(chunk: &usvg::TextChunk) -> bool {
    let spans = chunk.spans();
    let Some(first) = spans.first() else {
        return false;
    };
    spans.iter().all(|span| source_span_matches(span, first))
        && contiguous_source_spans(spans, chunk.text().len())
}

fn source_span_matches(span: &usvg::TextSpan, first: &usvg::TextSpan) -> bool {
    span.font() == first.font()
        && span.font_size().get() == first.font_size().get()
        && span.small_caps() == first.small_caps()
        && span.apply_kerning() == first.apply_kerning()
        && span.font_optical_sizing() == first.font_optical_sizing()
        && span.dominant_baseline() == first.dominant_baseline()
        && span.alignment_baseline() == first.alignment_baseline()
        && span.baseline_shift() == first.baseline_shift()
        && span.paint_order() == first.paint_order()
        && span.is_visible()
        && span.stroke().is_none()
        && decoration_supported(span)
        && super::super::alignment_source_span_supported(span)
}

fn decoration_supported(span: &usvg::TextSpan) -> bool {
    let decoration = span.decoration();
    decoration.underline().is_none()
        && decoration.overline().is_none()
        && decoration.line_through().is_none()
}

fn contiguous_source_spans(spans: &[usvg::TextSpan], text_len: usize) -> bool {
    let mut end = 0;
    for span in spans {
        if span.start() != end || span.end() <= end {
            return false;
        }
        end = span.end();
    }
    end == text_len
}

fn prepared_spans_supported(spans: &[PreparedSpan]) -> bool {
    let Some(first) = spans.first() else {
        return false;
    };
    spans.iter().all(|span| {
        span.font == first.font
            && span.font_size == first.font_size
            && span.upem == first.upem
            && span.baseline_shift == first.baseline_shift
            && span.fill == first.fill
            && !span.glyphs.is_empty()
            && super::super::horizontal_glyphs(&span.glyphs)
    })
}

fn layout_fragments_match(text: &usvg::Text, prepared: &[PreparedSpan]) -> bool {
    let mut layouts = text
        .layouted()
        .iter()
        .filter(|span| !span.positioned_glyphs.is_empty());
    for paint_span in prepared {
        let Some(layout) = layouts.next() else {
            return false;
        };
        let mut runs = super::super::runs::consecutive_runs(&layout.positioned_glyphs);
        let Some(run) = runs.next() else {
            return false;
        };
        if runs.next().is_some()
            || run.font != paint_span.font
            || run.font_size != paint_span.font_size
            || layout.positioned_glyphs.len() != paint_span.glyphs.len()
        {
            return false;
        }
    }
    layouts.next().is_none()
}

fn ordered_ltr_glyphs(chunk: &usvg::TextChunk, prepared: &[PreparedSpan]) -> bool {
    let mut characters = chunk.text().chars();
    let mut previous_x = None;
    for glyph in prepared.iter().flat_map(|span| &span.glyphs) {
        let Some(character) = characters.next() else {
            return false;
        };
        let mut glyph_text = glyph.text.chars();
        if glyph_text.next() != Some(character) || glyph_text.next().is_some() {
            return false;
        }
        let x = glyph.transform().tx;
        if !x.is_finite() || previous_x.is_some_and(|previous| x <= previous) {
            return false;
        }
        previous_x = Some(x);
    }
    characters.next().is_none()
}
