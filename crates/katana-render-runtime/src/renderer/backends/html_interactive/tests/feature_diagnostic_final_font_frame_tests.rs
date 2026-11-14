use serde_json::json;
#[path = "feature_diagnostic_final_font_frame_test_support.rs"]
mod test_support;
use test_support::{complete_font_audit, font_source, glyph_tree};

#[test]
fn catalog_font_provenance_uses_latest_root_and_embedded_glyph_trees() {
    let old = glyph_tree(1, "root", "Transient");
    let latest = glyph_tree(3, "root", "Visible");
    let embedded = glyph_tree(3, "root/image-svg@root/0", "Visible");
    let mut audit = complete_font_audit(3, "Visible");
    audit["finalGlyphTrees"] = json!([old, latest, embedded]);
    audit["faces"] = json!([font_source("Transient"), font_source("Visible")]);

    let records = super::catalog_font_provenance(&audit);
    assert_eq!(records.as_array().map(Vec::len), Some(1));
    assert_eq!(records[0]["family"], "Visible");
    assert!(super::catalog_font_audit_complete(&audit, &records));
}

#[test]
fn merge_keeps_all_paint_history_and_no_paint_operation_retains_initial_frame() {
    let merged = super::merge_font_audits(
        complete_font_audit(1, "Initial"),
        test_support::operation_with_intermediate_and_final(),
    );
    assert_merged_final_frame(&merged);
    assert_no_paint_retains_final_frame(merged);
}

fn assert_merged_final_frame(merged: &serde_json::Value) {
    assert_eq!(merged["finalGlyphTrees"].as_array().map(Vec::len), Some(3));
    assert_eq!(merged["finalGlyphTrees"][1]["paintCall"], 2);
    assert_eq!(merged["finalGlyphTrees"][2]["paintCall"], 3);
    assert_eq!(merged["finalGlyphPaintCalls"], 3);
    let records = super::catalog_font_provenance(merged);
    assert_eq!(records.as_array().map(Vec::len), Some(1));
    assert_eq!(records[0]["family"], "Final");
}

fn assert_no_paint_retains_final_frame(merged: serde_json::Value) {
    let retained = super::merge_font_audits(merged, test_support::no_paint_operation());
    let records = super::catalog_font_provenance(&retained);
    assert_eq!(records[0]["family"], "Final");
}

#[test]
fn final_glyph_errors_drops_unknown_faces_and_missing_or_overflowed_paints_fail_closed() {
    assert_font_audit_is_rejected(test_support::unknown_face_audit());
    assert_font_audit_is_rejected(test_support::dropped_glyph_audit());
    assert_font_audit_is_rejected(test_support::glyph_error_audit());
    assert_font_audit_is_rejected(test_support::missing_final_paint_audit());
    assert_font_audit_is_rejected(test_support::overflowed_paint_audit());
}

fn assert_font_audit_is_rejected(audit: serde_json::Value) {
    assert!(
        super::catalog_font_provenance(&audit)
            .as_array()
            .is_some_and(Vec::is_empty)
    );
}

#[test]
fn merge_missing_error_and_drop_counters_does_not_turn_them_into_zero() {
    let initial = complete_font_audit(1, "Initial");
    let mut operation = complete_font_audit(1, "Final");
    if let Some(object) = operation.as_object_mut() {
        object.remove("errors");
        object.remove("dropped");
    }
    let merged = super::merge_font_audits(initial, operation);
    assert!(!super::catalog_font_audit_complete(
        &merged,
        &super::catalog_font_provenance(&merged)
    ));
}
