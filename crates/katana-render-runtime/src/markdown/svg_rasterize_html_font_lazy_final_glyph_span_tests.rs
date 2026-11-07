use super::{MAX_FINAL_GLYPH_RECORDS, TreeEvidence, output};

pub(super) fn record_glyph_layout(
    span: &resvg::usvg::layout::Span,
    evidence: &mut TreeEvidence,
) -> Vec<output::FinalGlyphLayout> {
    let available = MAX_FINAL_GLYPH_RECORDS.saturating_sub(evidence.glyph_layout_recorded);
    let glyphs = span.positioned_glyphs.iter().take(available);
    let attempted = glyphs.clone().count();
    let mut rows = Vec::new();
    for glyph in glyphs {
        match output::FinalGlyphLayout::from_glyph(glyph) {
            Ok(row) => rows.push(row),
            Err(error) => {
                evidence.glyph_layout_errors.push(error);
            }
        }
    }
    evidence.glyph_layout_recorded += attempted;
    evidence.glyph_layout_dropped += span.positioned_glyphs.len().saturating_sub(rows.len());
    rows
}

pub(super) fn request_candidates(
    span: &resvg::usvg::layout::Span,
    requested: &[output::RequestedTextSpan],
) -> Vec<usize> {
    requested
        .iter()
        .enumerate()
        .filter_map(|(index, source)| {
            let text = source.text.as_deref()?;
            let contains_all_glyph_text = !span.positioned_glyphs.is_empty()
                && span
                    .positioned_glyphs
                    .iter()
                    .all(|glyph| !glyph.text.is_empty() && text.contains(glyph.text.as_str()));
            contains_all_glyph_text.then_some(index)
        })
        .collect()
}
