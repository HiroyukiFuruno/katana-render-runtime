use super::PreparedSpan;

pub(super) fn assert_identity_source(span: &PreparedSpan, event: &serde_json::Value) {
    let source = &event["sourceSpan"];
    assert_eq!(source["spanFontSize"], serde_json::json!(span.font_size));
    assert_eq!(source["rasterScale"], serde_json::json!(1.0));
    assert!(source["drawGlyphIndex"].is_null());
    assert_eq!(
        source["glyphs"].as_array().map(Vec::len),
        Some(span.glyphs.len())
    );
    for (index, glyph) in span.glyphs.iter().enumerate() {
        assert_eq!(
            source["glyphs"][index]["id"],
            serde_json::json!(span.glyph_ids[index])
        );
        assert_eq!(
            source["glyphs"][index]["fontSize"],
            serde_json::json!(glyph.font_size())
        );
    }
    assert_eq!(
        source["glyphs"][0]["relativePosition"],
        serde_json::json!([0.0, 0.0])
    );
}

pub(super) fn assert_transformed_source(index: usize, event: &serde_json::Value) {
    assert_eq!(
        event["sourceSpan"]["drawGlyphIndex"],
        serde_json::json!(index)
    );
    assert_eq!(
        event["glyphIds"][0],
        event["sourceSpan"]["glyphs"][index]["id"]
    );
}
