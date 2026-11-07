use resvg::usvg::{
    Group, ImageKind, Node, Text, Tree,
    fontdb::{Database, ID},
};
use std::collections::BTreeMap;
#[path = "svg_rasterize_html_font_lazy_final_glyph_faces.rs"]
mod faces;
#[path = "svg_rasterize_html_font_lazy_final_glyph_span_tests.rs"]
mod span;
use span::{record_glyph_layout, request_candidates};
#[path = "svg_rasterize_html_font_lazy_final_glyph_layout.rs"]
pub(super) mod layout;
#[path = "svg_rasterize_html_font_lazy_final_glyph_output.rs"]
mod output;
#[path = "svg_rasterize_html_font_lazy_final_variations.rs"]
mod variations;
pub(super) use output::{FinalGlyphError, FinalGlyphTree};

#[cfg(test)]
#[path = "svg_rasterize_html_font_lazy_final_glyph_css_equivalence_tests.rs"]
mod css_equivalence_tests;
#[cfg(test)]
#[path = "svg_rasterize_html_font_lazy_final_glyph_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "svg_rasterize_html_font_lazy_final_glyph_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "svg_rasterize_html_font_lazy_final_variations_tests.rs"]
mod variations_tests;

const MAX_FINAL_GLYPH_RECORDS: usize = 512;

#[derive(Default)]
struct TreeEvidence {
    faces: BTreeMap<ID, u64>,
    text_spans: Vec<output::FinalGlyphTextSpan>,
    embedded: Vec<(String, Tree)>,
    dropped: usize,
    glyph_layout_recorded: usize,
    glyph_layout_dropped: usize,
    glyph_layout_errors: Vec<String>,
}

pub(super) fn record(tree: &Tree, source_svg: &str, paint_call: u64) {
    let source_svg_sha256 = output::sha256(source_svg.as_bytes());
    record_tree(
        tree,
        source_svg_sha256,
        Some(source_svg.to_owned()),
        paint_call,
        "root".into(),
    );
}

fn record_tree(
    tree: &Tree,
    source_svg_sha256: String,
    source_svg: Option<String>,
    paint_call: u64,
    tree_path: String,
) {
    let mut evidence = TreeEvidence::default();
    walk_group(tree.root(), &tree_path, tree.fontdb(), &mut evidence);
    let resolved = faces::resolve(tree, &tree_path, paint_call, &evidence.faces);
    let glyph_layout_dropped = evidence.glyph_layout_dropped;
    let mut record = FinalGlyphTree {
        paint_call,
        tree_path: tree_path.clone(),
        source_svg_sha256: source_svg_sha256.clone(),
        source_svg,
        resolved_svg_sha256: None,
        resolved_svg: None,
        resolved_text_svg_sha256: None,
        resolved_text_svg: None,
        faces: resolved.faces,
        text_spans: evidence.text_spans,
        glyph_layout_dropped,
        glyph_layout_errors: evidence.glyph_layout_errors,
    };
    serialize_record_tree(tree, &mut record);
    output::append_tree(record, resolved.errors, resolved.owners, evidence.dropped);
    record_embedded(evidence.embedded, source_svg_sha256, paint_call, tree_path);
}

fn serialize_record_tree(tree: &Tree, record: &mut FinalGlyphTree) {
    if record.source_svg.is_none() {
        return;
    }
    let svg = output::serialize_tree(tree, false);
    let text_svg = output::serialize_tree(tree, true);
    record.resolved_svg_sha256 = Some(output::sha256(svg.as_bytes()));
    record.resolved_svg = Some(svg);
    record.resolved_text_svg_sha256 = Some(output::sha256(text_svg.as_bytes()));
    record.resolved_text_svg = Some(text_svg);
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
            None,
            paint_call,
            format!("{tree_path}/{suffix}"),
        );
    }
}

fn walk_group(group: &Group, path: &str, fontdb: &Database, evidence: &mut TreeEvidence) {
    for (index, node) in group.children().iter().enumerate() {
        let node_path = format!("{path}/{index}");
        walk_node(node, &node_path, fontdb, evidence);
    }
}

fn walk_node(node: &Node, path: &str, fontdb: &Database, evidence: &mut TreeEvidence) {
    match node {
        Node::Group(group) => walk_group(group, path, fontdb, evidence),
        Node::Text(text) => record_text_fonts(text, path, fontdb, evidence),
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
        walk_subroots(node, path, fontdb, evidence);
    }
}

fn record_text_fonts(text: &Text, path: &str, fontdb: &Database, evidence: &mut TreeEvidence) {
    let (requested, source_spans_dropped) = output::requested_text_spans(text);
    evidence.dropped += source_spans_dropped;
    for (layout_span_index, span) in text.layouted().iter().enumerate() {
        let span_faces = span_face_counts(span);
        for glyph in &span.positioned_glyphs {
            record_glyph_font(glyph.font, evidence);
        }
        record_text_span(TextSpanRecord {
            text,
            text_path: path,
            span,
            layout_span_index,
            requested: &requested,
            span_faces,
            fontdb,
            evidence,
        });
    }
}

fn span_face_counts(span: &resvg::usvg::layout::Span) -> BTreeMap<ID, u64> {
    let mut faces = BTreeMap::new();
    for glyph in &span.positioned_glyphs {
        *faces.entry(glyph.font).or_default() += 1;
    }
    faces
}

struct TextSpanRecord<'a> {
    text: &'a Text,
    text_path: &'a str,
    span: &'a resvg::usvg::layout::Span,
    layout_span_index: usize,
    requested: &'a [output::RequestedTextSpan],
    span_faces: BTreeMap<ID, u64>,
    fontdb: &'a Database,
    evidence: &'a mut TreeEvidence,
}

fn record_text_span(record: TextSpanRecord<'_>) {
    record.record();
}

impl TextSpanRecord<'_> {
    fn record(mut self) {
        if self.evidence.text_spans.len() >= MAX_FINAL_GLYPH_RECORDS {
            self.evidence.dropped += 1;
            return;
        }
        if !record_text_transform(self.text, self.text_path, self.span, self.evidence) {
            return;
        }
        let row = match text_span_evidence(&mut self) {
            Ok(row) => row,
            Err(error) => {
                self.evidence.glyph_layout_errors.push(error);
                self.evidence.glyph_layout_dropped += self.span.positioned_glyphs.len();
                return;
            }
        };
        self.evidence.text_spans.push(row);
    }
}

fn record_text_transform(
    text: &Text,
    text_path: &str,
    span: &resvg::usvg::layout::Span,
    evidence: &mut TreeEvidence,
) -> bool {
    let transform = output::transform_values(text.abs_transform());
    if output::transform_is_finite(transform) {
        return true;
    }
    evidence
        .glyph_layout_errors
        .push(format!("non-finite text transform at {text_path}"));
    evidence.glyph_layout_dropped += span.positioned_glyphs.len();
    false
}

fn text_span_evidence(
    record: &mut TextSpanRecord<'_>,
) -> Result<output::FinalGlyphTextSpan, String> {
    let span = record.span;
    let candidates = request_candidates(span, record.requested);
    let (association, request) = match candidates.as_slice() {
        [index] => ("unique_text_match", Some(record.requested[*index].clone())),
        [] => ("unmatched_text", None),
        _ => ("ambiguous_text_match", None),
    };
    let faces = span_face_evidence(record.fontdb, span, std::mem::take(&mut record.span_faces))?;
    Ok(output::FinalGlyphTextSpan {
        layout_span_index: record.layout_span_index,
        text_path: record.text_path.to_owned(),
        text_abs_transform: output::transform_values(record.text.abs_transform()),
        request_association: association,
        requested: request,
        request_candidate_indices: candidates,
        layout_variations: output::variation_evidence(&span.variations),
        glyph_count: span.positioned_glyphs.len(),
        faces,
        glyph_layout: record_glyph_layout(span, record.evidence),
    })
}

fn span_face_evidence(
    fontdb: &Database,
    span: &resvg::usvg::layout::Span,
    span_faces: BTreeMap<ID, u64>,
) -> Result<Vec<output::FinalGlyphSpanFace>, String> {
    span_faces
        .into_iter()
        .map(|(face_id, glyph_count)| {
            let glyph = span
                .positioned_glyphs
                .iter()
                .find(|glyph| glyph.font == face_id)
                .ok_or_else(|| format!("missing glyph for final font face {face_id:?}"))?;
            let (effective_variations, design_coordinates, normalized_coords) =
                variations::face_observation(fontdb, span, face_id, glyph)?;
            Ok(output::FinalGlyphSpanFace {
                face_id: face_id.to_string(),
                glyph_count,
                effective_variations: output::variation_evidence(&effective_variations),
                derived_applied_design_coordinates: output::variation_evidence(&design_coordinates),
                normalized_coords,
            })
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

fn walk_subroots(node: &Node, path: &str, fontdb: &Database, evidence: &mut TreeEvidence) {
    let mut index = 0;
    node.subroots(|root| {
        walk_group(root, &format!("{path}/subroot-{index}"), fontdb, evidence);
        index += 1;
    });
}
