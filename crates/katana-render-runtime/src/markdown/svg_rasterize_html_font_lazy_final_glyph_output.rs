use super::super::sources;
use resvg::usvg::{Text, Tree, WriteOptions};
use serde::Serialize;

#[path = "svg_rasterize_html_font_lazy_final_glyph_output_buffer.rs"]
mod buffer;
pub(super) use super::layout::{
    FinalGlyphLayout, TRANSFORM_COMPONENTS, ensure_finite_layout, transform_is_finite,
    transform_values,
};

pub(super) fn requested_text_spans(text: &Text) -> (Vec<RequestedTextSpan>, usize) {
    let total = text
        .chunks()
        .iter()
        .map(|chunk| chunk.spans().len())
        .sum::<usize>();
    let spans = text
        .chunks()
        .iter()
        .enumerate()
        .flat_map(|(chunk_index, chunk)| {
            chunk
                .spans()
                .iter()
                .enumerate()
                .map(move |(span_index, source_span)| {
                    requested_span(chunk_index, span_index, chunk, source_span)
                })
        })
        .take(super::MAX_FINAL_GLYPH_RECORDS)
        .collect();
    (spans, total.saturating_sub(super::MAX_FINAL_GLYPH_RECORDS))
}

fn requested_span(
    chunk_index: usize,
    span_index: usize,
    chunk: &resvg::usvg::TextChunk,
    source_span: &resvg::usvg::TextSpan,
) -> RequestedTextSpan {
    let source_text = chunk.text().get(source_span.start()..source_span.end());
    RequestedTextSpan {
        chunk_index,
        span_index,
        text: source_text.map(str::to_owned),
        families: source_span
            .font()
            .families()
            .iter()
            .map(|family| format!("{family:?}"))
            .collect(),
        weight: source_span.font().weight(),
        variations: variation_evidence(source_span.font().variations()),
    }
}

pub(super) fn variation_evidence(
    variations: &[resvg::usvg::FontVariation],
) -> Vec<FontVariationEvidence> {
    variations
        .iter()
        .map(|variation| FontVariationEvidence {
            tag: String::from_utf8_lossy(&variation.tag).into_owned(),
            value: variation.value,
        })
        .collect()
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct FinalGlyphTree {
    pub(super) paint_call: u64,
    pub(super) tree_path: String,
    pub(super) source_svg_sha256: String,
    pub(super) source_svg: Option<String>,
    pub(super) resolved_svg_sha256: Option<String>,
    pub(super) resolved_svg: Option<String>,
    pub(super) resolved_text_svg_sha256: Option<String>,
    pub(super) resolved_text_svg: Option<String>,
    pub(super) faces: Vec<FinalGlyphFace>,
    pub(super) text_spans: Vec<FinalGlyphTextSpan>,
    pub(super) glyph_layout_dropped: usize,
    pub(super) glyph_layout_errors: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FinalGlyphTextSpan {
    pub(super) layout_span_index: usize,
    pub(super) text_path: String,
    pub(super) text_abs_transform: [f32; TRANSFORM_COMPONENTS],
    pub(super) request_association: &'static str,
    pub(super) requested: Option<RequestedTextSpan>,
    pub(super) request_candidate_indices: Vec<usize>,
    pub(super) layout_variations: Vec<FontVariationEvidence>,
    pub(super) glyph_count: usize,
    pub(super) faces: Vec<FinalGlyphSpanFace>,
    pub(super) glyph_layout: Vec<FinalGlyphLayout>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RequestedTextSpan {
    pub(super) chunk_index: usize,
    pub(super) span_index: usize,
    pub(super) text: Option<String>,
    pub(super) families: Vec<String>,
    pub(super) weight: u16,
    pub(super) variations: Vec<FontVariationEvidence>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FontVariationEvidence {
    pub(super) tag: String,
    pub(super) value: f32,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FinalGlyphSpanFace {
    pub(super) face_id: String,
    pub(super) glyph_count: u64,
    pub(super) effective_variations: Vec<FontVariationEvidence>,
    pub(super) derived_applied_design_coordinates: Vec<FontVariationEvidence>,
    pub(super) normalized_coords: Vec<f32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FinalGlyphFace {
    pub(super) face_id: String,
    pub(super) glyph_count: u64,
    pub(super) identity: &'static str,
    pub(super) source: Option<sources::FaceSource>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in super::super) struct FinalGlyphError {
    pub(super) paint_call: u64,
    pub(super) tree_path: String,
    pub(super) face_id: Option<String>,
    pub(super) reason: String,
}
pub(super) use buffer::append_tree;

pub(super) fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn serialize_tree(tree: &Tree, preserve_text: bool) -> String {
    tree.to_string(&WriteOptions {
        preserve_text,
        ..Default::default()
    })
}
