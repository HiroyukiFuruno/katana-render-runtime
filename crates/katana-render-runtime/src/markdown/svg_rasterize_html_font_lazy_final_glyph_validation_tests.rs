use super::super as audit;
use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use audit::test_support::{font_tree, text_span};
use resvg::usvg::{
    Tree,
    fontdb::{Database, Source},
};
use std::collections::BTreeMap;
use std::sync::Arc;

#[test]
fn final_glyph_recording_rejects_a_removed_real_font_source() -> Result<(), String> {
    let tree = bundled_tree()?;
    let (_, span) = text_span(&tree)?;
    let face = span
        .positioned_glyphs
        .first()
        .ok_or("audit glyph is missing")?
        .font;
    let mut database = tree.fontdb().as_ref().clone();
    database.remove_face(face);
    let mut evidence = audit::TreeEvidence::default();
    record_first_span(&tree, &database, &mut evidence)?;
    assert!(evidence.text_spans.is_empty());
    assert_eq!(evidence.glyph_layout_errors.len(), 1);
    assert!(evidence.glyph_layout_errors[0].contains("font face data unavailable"));
    assert_eq!(evidence.glyph_layout_dropped, span.positioned_glyphs.len());
    assert!(evidence.glyph_layout_dropped > 0);
    Ok(())
}

#[test]
fn final_glyph_recording_stops_at_the_text_span_limit() -> Result<(), String> {
    let tree = bundled_tree()?;
    let mut evidence = audit::TreeEvidence::default();
    record_first_span(&tree, tree.fontdb(), &mut evidence)?;
    let row = evidence
        .text_spans
        .first()
        .ok_or("audit text row is missing")?
        .clone();
    evidence
        .text_spans
        .resize(audit::MAX_FINAL_GLYPH_RECORDS, row);
    let glyphs_recorded = evidence.glyph_layout_recorded;
    record_first_span(&tree, tree.fontdb(), &mut evidence)?;
    assert_eq!(evidence.text_spans.len(), audit::MAX_FINAL_GLYPH_RECORDS);
    assert_eq!(evidence.glyph_layout_recorded, glyphs_recorded);
    assert_eq!(evidence.dropped, 1);
    assert!(evidence.glyph_layout_errors.is_empty());
    Ok(())
}

#[test]
fn final_glyph_span_face_requires_a_matching_actual_glyph() -> Result<(), String> {
    let tree = bundled_tree()?;
    let (_, span) = text_span(&tree)?;
    let mut database = tree.fontdb().as_ref().clone();
    let loaded = database.load_font_source(Source::Binary(Arc::new(FONT_BYTES.to_vec())));
    let face = *loaded.first().ok_or("additional real face is missing")?;
    assert!(
        span.positioned_glyphs
            .iter()
            .all(|glyph| glyph.font != face)
    );
    let result = audit::span_face_evidence(&database, span, BTreeMap::from([(face, 1)]));
    assert!(result.is_err_and(|error| error.contains("missing glyph for final font face")));
    Ok(())
}

fn bundled_tree() -> Result<Tree, String> {
    let (database, _) = audit::variations_tests::bundled_face()?;
    font_tree(database)
}

fn record_first_span(
    tree: &Tree,
    database: &Database,
    evidence: &mut audit::TreeEvidence,
) -> Result<(), String> {
    let (text, span) = text_span(tree)?;
    audit::record_text_span(audit::TextSpanRecord {
        text,
        text_path: "root/audit-run",
        span,
        layout_span_index: 0,
        requested: &[],
        span_faces: audit::span_face_counts(span),
        fontdb: database,
        evidence,
    });
    Ok(())
}
