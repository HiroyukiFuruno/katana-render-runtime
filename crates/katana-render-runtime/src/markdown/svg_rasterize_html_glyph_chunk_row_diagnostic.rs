use super::{
    PreparedSpan, chunk_input, chunk_supported, first_supported_spans, glyphs_supported, row_input,
    row_inputs, rows_supported, source_supported,
};
use resvg::usvg;

pub(in crate::markdown::svg_rasterize) fn row_diagnostic(
    text: &usvg::Text,
    prepared: &[PreparedSpan],
) -> serde_json::Value {
    let mut span_offset = 0_usize;
    let mut character_offset = 0_usize;
    let rows: Vec<_> = text.chunks().iter().map(|chunk| {
        let span_end = span_offset + chunk.spans().len();
        let character_end = character_offset + chunk.text().chars().count();
        let spans = prepared.get(span_offset..span_end);
        let layouts = text.layouted().get(span_offset..span_end);
        let dy = text.dy().get(character_offset..character_end);
        let input = row_input(text, chunk, prepared, span_offset, character_offset).is_some();
        let detail = diagnostic_row(text, chunk, spans, layouts, dy);
        let result = serde_json::json!({"spanRange": [span_offset, span_end], "characterRange": [character_offset, character_end], "rowInput": input, "detail": detail});
        span_offset = span_end;
        character_offset = character_end;
        result
    }).collect();
    serde_json::json!({
        "chunkCount": text.chunks().len(), "layoutCount": text.layouted().len(),
        "preparedCount": prepared.len(), "dy": text.dy(), "dx": text.dx(), "rotate": text.rotate(),
        "rowsSupported": rows_supported(text, prepared), "rowInputs": row_inputs(text, prepared).is_some(),
        "consumedSpans": span_offset, "consumedCharacters": character_offset, "rows": rows,
    })
}

fn diagnostic_row(
    text: &usvg::Text,
    chunk: &usvg::TextChunk,
    spans: Option<&[PreparedSpan]>,
    layouts: Option<&[usvg::layout::Span]>,
    dy: Option<&[f32]>,
) -> serde_json::Value {
    serde_json::json!({
        "sourceCount": chunk.spans().len(), "text": chunk.text(), "anchor": format!("{:?}", chunk.anchor()),
        "chunkSupported": spans.map(|spans| chunk_supported(chunk, spans)),
        "spansSupported": spans.map(|spans| first_supported_spans(chunk, spans).is_some()),
        "chunkInput": spans.map(|spans| chunk_input(text, chunk, spans).is_some()),
        "cssScale": super::super::anchor::uniform_css_scale(text.abs_transform()),
        "paintSize": spans.and_then(|spans| spans.first()).and_then(|span| super::super::anchor::uniform_css_scale(text.abs_transform()).and_then(|scale| super::super::anchor::paint_size(span.font_size, scale, scale))),
        "dy": dy, "dySupported": dy.map(super::super::super::alignment_dy_supported),
        "layoutsMatch": layouts.zip(spans).map(|(layouts, spans)| layouts.iter().zip(spans).map(|(layout, span)| super::super::super::multi_chunk_alignment::layout_matches(layout, span)).collect::<Vec<_>>()),
        "spans": diagnostic_spans(chunk, spans),
    })
}

fn diagnostic_spans(
    chunk: &usvg::TextChunk,
    spans: Option<&[PreparedSpan]>,
) -> Option<Vec<serde_json::Value>> {
    spans.map(|spans| chunk.spans().iter().zip(spans).map(|(source, span)| {
        serde_json::json!({
            "range": [source.start(), source.end()], "fontSize": span.font_size,
            "font": format!("{:?}", span.font), "glyphs": span.glyphs.len(),
            "glyphFontSizes": span.glyphs.iter().map(|glyph| glyph.font_size()).collect::<Vec<_>>(),
            "sameSourceFont": chunk.spans().first().map(|first| source.font() == first.font()),
            "sameFace": spans.first().map(|first| span.font == first.font),
            "sameSize": spans.first().map(|first| span.font_size == first.font_size),
            "sameBaseline": spans.first().map(|first| span.baseline_shift == first.baseline_shift),
            "sourceSupported": source_supported(source),
            "glyphsSupported": glyphs_supported(chunk, source, span),
            "xAlignment": span.x_alignment.is_some(), "chunkAlignment": span.chunk_alignment.is_some(),
        })
    }).collect::<Vec<_>>())
}

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_chunk_row_diagnostic_tests.rs"]
mod tests;
