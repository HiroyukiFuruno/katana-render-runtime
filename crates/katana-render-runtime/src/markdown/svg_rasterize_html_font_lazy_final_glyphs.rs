use resvg::usvg::{Group, ImageKind, Node, Text, Tree, fontdb::ID};
use std::collections::BTreeMap;
#[path = "svg_rasterize_html_font_lazy_final_glyph_faces.rs"]
mod faces;
#[path = "svg_rasterize_html_font_lazy_final_glyph_output.rs"]
mod output;
pub(super) use output::{FinalGlyphError, FinalGlyphTree};

#[cfg(test)]
#[path = "svg_rasterize_html_font_lazy_final_glyph_css_equivalence_tests.rs"]
mod css_equivalence_tests;
#[cfg(test)]
#[path = "svg_rasterize_html_font_lazy_final_glyph_tests.rs"]
mod tests;

const MAX_FINAL_GLYPH_RECORDS: usize = 512;

#[derive(Default)]
struct TreeEvidence {
    faces: BTreeMap<ID, u64>,
    text_spans: Vec<output::FinalGlyphTextSpan>,
    embedded: Vec<(String, Tree)>,
    dropped: usize,
}

pub(super) fn record(tree: &Tree, source_svg: &str, paint_call: Option<u64>) {
    let Some(paint_call) = paint_call else {
        return;
    };
    let source_svg_sha256 = output::sha256(source_svg.as_bytes());
    record_tree(tree, source_svg_sha256, paint_call, "root".into());
}

fn record_tree(tree: &Tree, source_svg_sha256: String, paint_call: u64, tree_path: String) {
    let mut evidence = TreeEvidence::default();
    walk_group(tree.root(), &tree_path, &mut evidence);
    let resolved = faces::resolve(tree, &tree_path, paint_call, &evidence.faces);
    let dropped = evidence.dropped;
    let text_spans = evidence.text_spans;
    output::append_tree(
        FinalGlyphTree {
            paint_call,
            tree_path: tree_path.clone(),
            source_svg_sha256: source_svg_sha256.clone(),
            faces: resolved.faces,
            text_spans,
        },
        resolved.errors,
        resolved.owners,
        dropped,
    );
    record_embedded(evidence.embedded, source_svg_sha256, paint_call, tree_path);
}

fn record_embedded(
    embedded_trees: Vec<(String, Tree)>,
    source_svg_sha256: String,
    paint_call: u64,
    tree_path: String,
) {
    for (suffix, embedded) in embedded_trees {
        record_tree(
            &embedded,
            source_svg_sha256.clone(),
            paint_call,
            format!("{tree_path}/{suffix}"),
        );
    }
}

fn walk_group(group: &Group, path: &str, evidence: &mut TreeEvidence) {
    for (index, node) in group.children().iter().enumerate() {
        let node_path = format!("{path}/{index}");
        walk_node(node, &node_path, evidence);
    }
}

fn walk_node(node: &Node, path: &str, evidence: &mut TreeEvidence) {
    match node {
        Node::Group(group) => walk_group(group, path, evidence),
        Node::Text(text) => record_text_fonts(text, evidence),
        Node::Image(image) => {
            if let ImageKind::SVG(embedded) = image.kind() {
                evidence
                    .embedded
                    .push((format!("image-svg@{path}"), embedded.clone()));
            }
        }
        Node::Path(_) => {}
    }
    if !matches!(node, Node::Image(image) if matches!(image.kind(), ImageKind::SVG(_))) {
        walk_subroots(node, path, evidence);
    }
}

fn record_text_fonts(text: &Text, evidence: &mut TreeEvidence) {
    let (requested, source_spans_dropped) = requested_text_spans(text);
    evidence.dropped += source_spans_dropped;
    for (layout_span_index, span) in text.layouted().iter().enumerate() {
        let span_faces = span_face_counts(span);
        for glyph in &span.positioned_glyphs {
            record_glyph_font(glyph.font, evidence);
        }
        record_text_span(span, layout_span_index, &requested, span_faces, evidence);
    }
}

fn requested_text_spans(text: &Text) -> (Vec<output::RequestedTextSpan>, usize) {
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
        .take(MAX_FINAL_GLYPH_RECORDS)
        .collect();
    (spans, total.saturating_sub(MAX_FINAL_GLYPH_RECORDS))
}

fn requested_span(
    chunk_index: usize,
    span_index: usize,
    chunk: &resvg::usvg::TextChunk,
    source_span: &resvg::usvg::TextSpan,
) -> output::RequestedTextSpan {
    let source_text = chunk.text().get(source_span.start()..source_span.end());
    output::RequestedTextSpan {
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

fn span_face_counts(span: &resvg::usvg::layout::Span) -> BTreeMap<ID, u64> {
    let mut faces = BTreeMap::new();
    for glyph in &span.positioned_glyphs {
        *faces.entry(glyph.font).or_default() += 1;
    }
    faces
}

fn record_text_span(
    span: &resvg::usvg::layout::Span,
    layout_span_index: usize,
    requested: &[output::RequestedTextSpan],
    span_faces: BTreeMap<ID, u64>,
    evidence: &mut TreeEvidence,
) {
    if evidence.text_spans.len() >= MAX_FINAL_GLYPH_RECORDS {
        evidence.dropped += 1;
        return;
    }
    let row = text_span_evidence(span, layout_span_index, requested, span_faces);
    evidence.text_spans.push(row);
}

fn text_span_evidence(
    span: &resvg::usvg::layout::Span,
    layout_span_index: usize,
    requested: &[output::RequestedTextSpan],
    span_faces: BTreeMap<ID, u64>,
) -> output::FinalGlyphTextSpan {
    let candidates = request_candidates(span, requested);
    let (association, request) = match candidates.as_slice() {
        [index] => ("unique_text_match", Some(requested[*index].clone())),
        [] => ("unmatched_text", None),
        _ => ("ambiguous_text_match", None),
    };
    output::FinalGlyphTextSpan {
        layout_span_index,
        request_association: association,
        requested: request,
        request_candidate_indices: candidates,
        layout_variations: variation_evidence(&span.variations),
        glyph_count: span.positioned_glyphs.len(),
        faces: span_faces
            .into_iter()
            .map(|(face_id, glyph_count)| output::FinalGlyphSpanFace {
                face_id: face_id.to_string(),
                glyph_count,
            })
            .collect(),
    }
}

fn request_candidates(
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

fn variation_evidence(
    variations: &[resvg::usvg::FontVariation],
) -> Vec<output::FontVariationEvidence> {
    variations
        .iter()
        .map(|variation| output::FontVariationEvidence {
            tag: String::from_utf8_lossy(&variation.tag).into_owned(),
            value: variation.value,
        })
        .collect()
}

fn record_glyph_font(font: ID, evidence: &mut TreeEvidence) {
    if evidence.faces.contains_key(&font) || evidence.faces.len() < MAX_FINAL_GLYPH_RECORDS {
        *evidence.faces.entry(font).or_default() += 1;
    } else {
        evidence.dropped += 1;
    }
}

fn walk_subroots(node: &Node, path: &str, evidence: &mut TreeEvidence) {
    let mut index = 0;
    node.subroots(|root| {
        walk_group(root, &format!("{path}/subroot-{index}"), evidence);
        index += 1;
    });
}
