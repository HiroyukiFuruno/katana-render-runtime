use serde_json::{Value, json};

pub(super) fn font_source(family: &str) -> Value {
    json!({
        "kind":"File", "path":"/font.ttf",
        "bytes_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "index":0, "families":[family]
    })
}

pub(super) fn glyph_tree(paint_call: u64, tree_path: &str, family: &str) -> Value {
    json!({
        "paintCall":paint_call, "treePath":tree_path,
        "faces":[{
            "identity":"physical_verified", "glyphCount":3,
            "source":font_source(family)
        }]
    })
}

pub(super) fn complete_font_audit(paint_calls: u64, family: &str) -> Value {
    json!({
        "faces":[font_source(family)], "errors":[], "dropped":0,
        "finalGlyphTrees":[glyph_tree(paint_calls, "root", family)],
        "finalGlyphErrors":[], "finalGlyphDropped":0,
        "finalGlyphPaintCalls":paint_calls
    })
}

pub(super) fn operation_with_intermediate_and_final() -> Value {
    json!({
        "faces":[font_source("Intermediate"), font_source("Final")],
        "errors":[], "dropped":0,
        "finalGlyphTrees":[
            glyph_tree(1, "root", "Intermediate"),
            glyph_tree(2, "root", "Final")
        ],
        "finalGlyphErrors":[], "finalGlyphDropped":0,
        "finalGlyphPaintCalls":2
    })
}

pub(super) fn no_paint_operation() -> Value {
    json!({
        "faces":[], "errors":[], "dropped":0,
        "finalGlyphTrees":[], "finalGlyphErrors":[], "finalGlyphDropped":0,
        "finalGlyphPaintCalls":0
    })
}

pub(super) fn unknown_face_audit() -> Value {
    let mut audit = complete_font_audit(1, "Example");
    audit["finalGlyphTrees"][0]["faces"][0]["identity"] = json!("unknown");
    audit
}

pub(super) fn dropped_glyph_audit() -> Value {
    let mut audit = complete_font_audit(1, "Example");
    audit["finalGlyphDropped"] = json!(1);
    audit
}

pub(super) fn glyph_error_audit() -> Value {
    let mut audit = complete_font_audit(1, "Example");
    audit["finalGlyphErrors"] = json!([{"paintCall":1,"reason":"unknown face"}]);
    audit
}

pub(super) fn missing_final_paint_audit() -> Value {
    let mut audit = complete_font_audit(1, "Example");
    audit["finalGlyphPaintCalls"] = json!(2);
    audit
}

pub(super) fn overflowed_paint_audit() -> Value {
    let mut initial = complete_font_audit(u64::MAX, "Initial");
    initial["finalGlyphTrees"][0]["paintCall"] = json!(u64::MAX);
    super::super::merge_font_audits(
        initial,
        json!({
            "faces":[], "errors":[], "dropped":0,
            "finalGlyphTrees":[glyph_tree(1, "root", "Overflow")],
            "finalGlyphErrors":[], "finalGlyphDropped":0,
            "finalGlyphPaintCalls":1
        }),
    )
}
