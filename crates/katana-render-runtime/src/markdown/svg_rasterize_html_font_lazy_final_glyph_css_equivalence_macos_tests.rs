use super::{
    BOLD_WEIGHT, FINAL_GLYPH_ORIGIN, NORMAL_WEIGHT, VIEWPORT_HEIGHT, VIEWPORT_WIDTH,
    assert_html_frame, assert_valid_paint_evidence, open_html_with_audit,
};

#[test]
fn actual_layout_regular_system_ui_raster_uses_w4_and_latin_sfns() -> Result<(), String> {
    assert_actual_layout_weight_faces(NORMAL_WEIGHT, ".HiraKakuInterface-W4")
}

#[test]
fn actual_layout_bold_system_ui_raster_uses_w6_and_latin_sfns() -> Result<(), String> {
    assert_actual_layout_weight_faces(BOLD_WEIGHT, ".HiraKakuInterface-W6")
}

fn assert_actual_layout_weight_faces(
    weight: u64,
    interface_postscript: &str,
) -> Result<(), String> {
    let html = weighted_system_ui_html(weight);
    let (session, audit) =
        open_html_with_audit(&html, FINAL_GLYPH_ORIGIN, VIEWPORT_WIDTH, VIEWPORT_HEIGHT)?;
    assert_html_frame(&session)?;
    assert_valid_paint_evidence(&audit)?;
    assert_weight_and_final_faces(&audit, weight, interface_postscript)
}

fn weighted_system_ui_html(weight: u64) -> String {
    format!(
        "<!doctype html><html lang=\"ja\"><body><p style=\"font-family:system-ui,sans-serif;font-weight:{weight}\">A日本</p></body></html>"
    )
}

fn assert_weight_and_final_faces(
    audit: &serde_json::Value,
    weight: u64,
    interface_postscript: &str,
) -> Result<(), String> {
    assert_computed_weight(audit, weight)?;
    let trees = audit["finalGlyphTrees"]
        .as_array()
        .ok_or_else(|| "final glyph tree evidence is absent".to_owned())?;
    assert_eq!(audit["finalGlyphDropped"], 0);
    assert_eq!(audit["finalGlyphErrors"], serde_json::json!([]));
    assert!(trees.iter().all(|tree| {
        tree["glyphLayoutDropped"] == 0 && tree["glyphLayoutErrors"] == serde_json::json!([])
    }));
    assert!(final_span_uses_face(trees, weight, interface_postscript));
    assert!(final_span_uses_face_prefix(trees, weight, ".SFNS"));
    Ok(())
}

fn assert_computed_weight(audit: &serde_json::Value, weight: u64) -> Result<(), String> {
    let markup = audit["htmlFontMarkups"]
        .as_array()
        .and_then(|markups| {
            markups.iter().find(|row| {
                row["computedWeight"] == weight && row["resolvedRunStatus"] == "resolved"
            })
        })
        .ok_or_else(|| format!("layout did not resolve font weight {weight}"))?;
    assert_eq!(markup["effectiveSvgWeight"], weight);
    Ok(())
}

fn final_span_uses_face(trees: &[serde_json::Value], weight: u64, postscript: &str) -> bool {
    final_span_uses_face_matching(trees, weight, |name| name == postscript, None)
}

fn final_span_uses_face_prefix(trees: &[serde_json::Value], weight: u64, prefix: &str) -> bool {
    let Some(actual_wght) = u16::try_from(weight).ok().map(f32::from) else {
        return false;
    };
    final_span_uses_face_matching(
        trees,
        weight,
        |name| name.starts_with(prefix),
        Some(actual_wght),
    )
}

fn final_span_uses_face_matching(
    trees: &[serde_json::Value],
    weight: u64,
    matches_name: impl Fn(&str) -> bool,
    actual_wght: Option<f32>,
) -> bool {
    trees
        .iter()
        .any(|tree| tree_has_weighted_face(tree, weight, &matches_name, actual_wght))
}

fn tree_has_weighted_face(
    tree: &serde_json::Value,
    weight: u64,
    matches_name: &impl Fn(&str) -> bool,
    actual_wght: Option<f32>,
) -> bool {
    let Some(faces) = tree["faces"].as_array() else {
        return false;
    };
    faces.iter().any(|face| {
        face["glyphCount"].as_u64().is_some_and(|count| count > 0)
            && face["source"]["post_script_name"]
                .as_str()
                .is_some_and(matches_name)
            && tree_span_uses_face(tree, &face["faceId"], weight, actual_wght)
    })
}

fn tree_span_uses_face(
    tree: &serde_json::Value,
    face_id: &serde_json::Value,
    weight: u64,
    actual_wght: Option<f32>,
) -> bool {
    tree["textSpans"].as_array().is_some_and(|spans| {
        spans.iter().any(|span| {
            span["requested"]["weight"] == weight
                && span["glyphCount"].as_u64().is_some_and(|count| count > 0)
                && actual_wght.is_none_or(|expected| span_has_wght(span, expected))
                && span["faces"].as_array().is_some_and(|faces| {
                    faces
                        .iter()
                        .any(|span_face| span_face["faceId"] == *face_id)
                })
        })
    })
}

fn span_has_wght(span: &serde_json::Value, expected: f32) -> bool {
    span["layoutVariations"]
        .as_array()
        .is_some_and(|variations| {
            variations.iter().any(|variation| {
                variation["tag"] == "wght"
                    && variation["value"].as_f64() == Some(f64::from(expected))
            })
        })
}
