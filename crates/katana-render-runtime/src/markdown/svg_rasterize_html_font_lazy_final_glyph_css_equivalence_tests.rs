use crate::markdown::svg_rasterize::SvgRasterizeOps;

const VIEWPORT_WIDTH: u32 = 900;
const VIEWPORT_HEIGHT: u32 = 600;
const NORMAL_WEIGHT: u64 = 400;
const BOLD_WEIGHT: u64 = 700;

const HTML: &str = r#"<!doctype html><html lang="ja"><head><style>
body { font-family: system-ui, sans-serif; }
h1 { font-size: 28px; font-weight: 700; }
p { font-size: 18px; font-weight: 400; }
strong { font-weight: 700; }
</style></head><body><h1>日本語の見出し</h1><p>通常の本文と<strong>太字の本文</strong>を描画します。</p></body></html>"#;
const INLINE_CONTROL_HTML: &str = r#"<!doctype html><html lang="ja"><body style="font-family:system-ui,sans-serif"><h1 style="font-size:28px;font-weight:700">日本語の見出し</h1><p style="font-size:18px;font-weight:400">通常の本文と<strong style="font-weight:700">太字の本文</strong>を描画します。</p></body></html>"#;

#[test]
fn actual_html_session_records_final_glyph_fonts() -> Result<(), String> {
    let (session, mut audit) = open_html_with_audit(HTML)?;
    let (control_session, control_audit) = open_html_with_audit(INLINE_CONTROL_HTML)?;
    let family_matches = record_family_comparison(&mut audit, &control_audit);
    audit["sourceCssFontFamily"] = serde_json::json!("system-ui, sans-serif");
    audit["inlineControlSourceCssFontFamily"] = serde_json::json!("system-ui,sans-serif");
    audit["rgbaComparison"] = rgba_comparison(&session, &control_session);
    audit["inlineControl"] = control_audit;
    println!("KRR_FINAL_GLYPH_HTML_AUDIT {audit}");
    assert_html_frame(&session)?;
    assert_valid_paint_evidence(&audit)?;
    assert_weighted_span_records(&audit)?;
    assert_html_frame(&control_session)?;
    assert_valid_paint_evidence(&audit["inlineControl"])?;
    assert_weighted_span_records(&audit["inlineControl"])?;
    assert_eq!(
        family_matches,
        Some(true),
        "style-block and inline families must match for 400 and 700 weights"
    );
    assert_eq!(
        audit["rgbaComparison"]["equal"], true,
        "equivalent CSS inputs must render identical RGBA frames"
    );
    Ok(())
}

fn assert_weighted_span_records(audit: &serde_json::Value) -> Result<(), String> {
    let trees = audit["finalGlyphTrees"]
        .as_array()
        .ok_or_else(|| "final glyph tree evidence is absent".to_owned())?;
    for weight in [NORMAL_WEIGHT, BOLD_WEIGHT] {
        let expected_family = resolved_family_name(audit, weight)?;
        let found = final_span_has_weighted_glyph(trees, weight, &expected_family);
        if !found {
            return Err(format!(
                "no uniquely paired final glyph span for weight {weight}"
            ));
        }
    }
    Ok(())
}

fn final_span_has_weighted_glyph(
    trees: &[serde_json::Value],
    weight: u64,
    expected_family: &str,
) -> bool {
    trees.iter().any(|tree| {
        tree["textSpans"].as_array().is_some_and(|spans| {
            spans.iter().any(|span| {
                span["requestAssociation"] == "unique_text_match"
                    && span["requested"]["weight"] == weight
                    && span["requested"]["families"]
                        .as_array()
                        .is_some_and(|families| {
                            families.contains(&serde_json::json!(expected_family))
                        })
                    && span["glyphCount"].as_u64().is_some_and(|count| count > 0)
                    && span["faces"]
                        .as_array()
                        .is_some_and(|faces| !faces.is_empty())
            })
        })
    })
}

fn resolved_family_name(audit: &serde_json::Value, weight: u64) -> Result<String, String> {
    audit["htmlFontMarkups"]
        .as_array()
        .and_then(|markups| {
            markups.iter().find(|row| {
                row["computedWeight"] == weight && row["resolvedRunStatus"] == "resolved"
            })
        })
        .and_then(|row| row["runFamilies"].as_array())
        .and_then(|families| families.first())
        .and_then(serde_json::Value::as_str)
        .map(|family| format!("Named({family:?})"))
        .ok_or_else(|| format!("no resolved HTML family for weight {weight}"))
}

fn record_family_comparison(
    audit: &mut serde_json::Value,
    control_audit: &serde_json::Value,
) -> Option<bool> {
    let weights = [NORMAL_WEIGHT, BOLD_WEIGHT].map(|weight| {
        let original = selector_family_for_weight(audit, weight);
        let control = selector_family_for_weight(control_audit, weight);
        serde_json::json!({"weight": weight, "originalStyleBlock": original, "inlineControl": control, "matches": original == control})
    });
    let all_match = weights.iter().all(|entry| entry["matches"] == true);
    let complete = weights
        .iter()
        .all(|entry| !entry["originalStyleBlock"].is_null() && !entry["inlineControl"].is_null());
    audit["familyComparison"] = serde_json::json!({"byWeight": weights, "allMatch": all_match});
    complete.then_some(all_match)
}

fn rgba_comparison(
    original: &crate::HtmlBrowserSession,
    control: &crate::HtmlBrowserSession,
) -> serde_json::Value {
    let original_pixels = original.latest_frame().map(|frame| frame.pixels.as_slice());
    let control_pixels = control.latest_frame().map(|frame| frame.pixels.as_slice());
    let original_sha = original_pixels.map(super::output::sha256);
    let control_sha = control_pixels.map(super::output::sha256);
    serde_json::json!({
        "originalSha256": original_sha,
        "inlineControlSha256": control_sha,
        "equal": original_pixels.zip(control_pixels).is_some_and(|(left, right)| left == right),
    })
}

fn selector_family_for_weight(audit: &serde_json::Value, weight: u64) -> Option<serde_json::Value> {
    audit["selectorEvents"]
        .as_array()?
        .iter()
        .find(|event| event["stage"] == "selector" && event["weight"] == weight)?
        .get("families")
        .cloned()
}

fn open_html_with_audit(
    html: &str,
) -> Result<(crate::HtmlBrowserSession, serde_json::Value), String> {
    let observer = crate::markdown::svg_rasterize::font::observation::Scope::enter();
    let source = crate::HtmlBrowserSource::new(html, "file:///krr-final-glyph-audit.html")
        .map_err(|error| error.to_string())?;
    let viewport = crate::HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())?;
    let (session, mut audit) = SvgRasterizeOps::with_actual_font_source_audit(|| {
        crate::HtmlRuntime.open(source, viewport)
    })
    .map_err(|error| error.to_string())?;
    audit["selectorEvents"] = serde_json::json!(observer.snapshot().events);
    Ok((session.map_err(|error| error.to_string())?, audit))
}

fn assert_html_frame(session: &crate::HtmlBrowserSession) -> Result<(), String> {
    let frame = session
        .latest_frame()
        .ok_or_else(|| "HTML session produced no initial frame".to_owned())?;
    assert_eq!(
        (frame.viewport.width, frame.viewport.height),
        (VIEWPORT_WIDTH, VIEWPORT_HEIGHT)
    );
    assert_eq!(
        frame.pixels.len(),
        VIEWPORT_WIDTH as usize * VIEWPORT_HEIGHT as usize * 4
    );
    assert!(frame.pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
    Ok(())
}

fn assert_valid_paint_evidence(audit: &serde_json::Value) -> Result<(), String> {
    let calls = audit["finalGlyphPaintCalls"]
        .as_u64()
        .filter(|calls| *calls > 0)
        .ok_or_else(|| "HTML session produced no observed SVG paint calls".to_owned())?;
    let trees = audit["finalGlyphTrees"]
        .as_array()
        .filter(|trees| !trees.is_empty())
        .ok_or_else(|| "HTML session produced no final glyph tree records".to_owned())?;
    let has_glyphs = trees.iter().any(|tree| {
        tree["faces"].as_array().is_some_and(|faces| {
            faces
                .iter()
                .any(|face| face["glyphCount"].as_u64().is_some_and(|count| count > 0))
        })
    });
    assert!(
        has_glyphs,
        "HTML session glyph trees contain no observed glyphs"
    );
    assert!(trees.iter().all(|tree| {
        tree["paintCall"]
            .as_u64()
            .is_some_and(|call| (1..=calls).contains(&call))
    }));
    Ok(())
}
