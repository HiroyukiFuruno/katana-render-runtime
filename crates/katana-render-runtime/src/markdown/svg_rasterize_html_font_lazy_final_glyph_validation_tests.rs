use super::super as audit;
use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use audit::test_support::{font_tree, text_span};
use resvg::usvg::{
    Group, Node, Transform, Tree,
    fontdb::{Database, Source},
};
use std::collections::BTreeMap;
use std::sync::Arc;

const NON_FINITE_SCALE: f32 = f32::NAN;
const ZERO_TRANSFORM_COMPONENT: f32 = 0.0;
const IDENTITY_TRANSFORM_COMPONENT: f32 = 1.0;

#[test]
fn audit_text_span_rejects_missing_or_nontext_id_in_real_svg() -> Result<(), String> {
    let reference = bundled_tree()?;
    assert!(!text_span(&reference)?.1.positioned_glyphs.is_empty());
    let options = resvg::usvg::Options {
        font_family: "Noto Sans".to_owned(),
        fontdb: Arc::clone(reference.fontdb()),
        ..Default::default()
    };
    for body in [
        "<text id='other' y='20'>AV</text>",
        "<rect id='audit-run' width='4' height='4'/><text y='20'>AV</text>",
    ] {
        let svg =
            format!("<svg xmlns='http://www.w3.org/2000/svg' width='64' height='32'>{body}</svg>");
        let tree = Tree::from_str(&svg, &options).map_err(|error| error.to_string())?;
        assert_eq!(
            text_span(&tree).err().as_deref(),
            Some("font audit fixture text is missing")
        );
    }
    Ok(())
}

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
fn final_glyph_recording_rejects_a_non_finite_text_transform() -> Result<(), String> {
    let mut tree = bundled_tree()?;
    let (_, span) = text_span(&tree)?;
    let expected_dropped = span.positioned_glyphs.len();
    assert!(expected_dropped > 0);
    let transform = Transform::from_row(
        NON_FINITE_SCALE,
        ZERO_TRANSFORM_COMPONENT,
        ZERO_TRANSFORM_COMPONENT,
        IDENTITY_TRANSFORM_COMPONENT,
        ZERO_TRANSFORM_COMPONENT,
        ZERO_TRANSFORM_COMPONENT,
    );
    if !replace_audit_text_transform(&mut tree.root, transform) {
        return Err("audit text transform could not be replaced".to_owned());
    }

    let mut evidence = audit::TreeEvidence::default();
    record_first_span(&tree, tree.fontdb(), &mut evidence)?;
    assert!(evidence.text_spans.is_empty());
    assert_eq!(evidence.glyph_layout_errors.len(), 1);
    assert_eq!(
        evidence.glyph_layout_errors[0],
        "non-finite text transform at root/audit-run"
    );
    assert_eq!(evidence.glyph_layout_dropped, expected_dropped);
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

fn replace_audit_text_transform(group: &mut Group, transform: Transform) -> bool {
    for node in &mut group.children {
        match node {
            Node::Text(text) if text.id() == "audit-run" => {
                text.abs_transform = transform;
                return true;
            }
            Node::Group(child) => {
                if replace_audit_text_transform(child, transform) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

#[test]
fn final_glyph_capture_truncates_faces_and_stops_at_capacity() -> Result<(), String> {
    use crate::markdown::svg_rasterize::SvgRasterizeOps;

    let empty_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="24"/>"#;
    let empty = Tree::from_str(empty_svg, &resvg::usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let tree = bundled_tree()?;
    let source = tree.to_string(&resvg::usvg::WriteOptions {
        preserve_text: true,
        ..Default::default()
    });
    /* WHY: バイナリフォントの通常記録はツリー・face・物理由来不明のエラーの3枠を使う。 */
    const RECORDS_PER_TREE: usize = 3;
    let empty_rows = (audit::MAX_FINAL_GLYPH_RECORDS - 1) % RECORDS_PER_TREE;
    let full_rows = (audit::MAX_FINAL_GLYPH_RECORDS - 1 - empty_rows) / RECORDS_PER_TREE;
    let (_, capture) = SvgRasterizeOps::with_actual_font_source_audit(|| {
        for paint_call in 0..empty_rows {
            audit::record(&empty, empty_svg, paint_call as u64);
        }
        for paint_call in empty_rows..(empty_rows + full_rows + 2) {
            audit::record(&tree, &source, paint_call as u64);
        }
    })
    .map_err(|error| error.to_string())?;
    assert_glyph_capacity_boundary(&capture, empty_rows, full_rows)
}

fn assert_glyph_capacity_boundary(
    capture: &serde_json::Value,
    empty_rows: usize,
    full_rows: usize,
) -> Result<(), String> {
    let trees = capture["finalGlyphTrees"]
        .as_array()
        .ok_or("glyph trees missing")?;
    assert_eq!(trees.len(), empty_rows + full_rows + 1);
    for tree in &trees[..empty_rows] {
        assert_eq!(tree["faces"].as_array().map(Vec::len), Some(0));
    }
    for tree in &trees[empty_rows..trees.len() - 1] {
        assert_eq!(tree["faces"].as_array().map(Vec::len), Some(1));
    }
    assert_eq!(
        trees
            .last()
            .and_then(|tree| tree["faces"].as_array())
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        capture["finalGlyphErrors"].as_array().map(Vec::len),
        Some(full_rows)
    );
    assert_eq!(capture["finalGlyphDropped"], 5);
    Ok(())
}
