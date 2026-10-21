use super::test_support::{
    audit_svg, root_tree, svg_with_many_text_spans, svg_with_repeated_glyph_text,
};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use resvg::usvg::fontdb::{Database, Source};
use std::sync::Arc;

#[test]
fn actual_raster_route_retains_each_top_and_embedded_svg_tree() -> Result<(), String> {
    let (first, second) = svg_inputs();
    let (renders, audit) = SvgRasterizeOps::with_actual_font_source_audit(|| {
        SvgRasterizeOps::rasterize_html_svg(&first, 1.0)?;
        SvgRasterizeOps::rasterize_html_svg(&second, 1.0)
    })
    .map_err(|error| error.to_string())?;
    renders.map_err(|error| error.to_string())?;
    assert_svg_tree_rows(&audit, &first, &second)?;
    assert_eq!(audit["finalGlyphDropped"], 0);
    Ok(())
}

#[test]
fn actual_raster_route_bounds_glyph_layout_sidecar() -> Result<(), String> {
    const EXTRA_GLYPHS: usize = 32;
    let text = "A".repeat(super::MAX_FINAL_GLYPH_RECORDS / 2 + EXTRA_GLYPHS);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="4096" height="32"><text x="1" y="20" font-family="Arial" font-size="16"><tspan>{text}</tspan><tspan x="2048">{text}</tspan></text></svg>"#
    );
    let (renders, audit) = SvgRasterizeOps::with_actual_font_source_audit(|| {
        SvgRasterizeOps::rasterize_html_svg(&svg, 1.0)
    })
    .map_err(|error| error.to_string())?;
    renders.map_err(|error| error.to_string())?;
    let trees = audit["finalGlyphTrees"]
        .as_array()
        .ok_or("glyph tree evidence is absent")?;
    let tree = trees
        .iter()
        .find(|tree| tree["treePath"] == "root")
        .ok_or("root glyph tree evidence is absent")?;
    assert_glyph_cap(tree)?;
    assert_eq!(audit["finalGlyphDropped"], 0);
    Ok(())
}

#[test]
fn actual_raster_route_bounds_layout_text_spans() -> Result<(), String> {
    let svg = svg_with_many_text_spans();
    let audit = audit_svg(&svg)?;
    let tree = root_tree(&audit)?;
    assert_eq!(
        tree["textSpans"].as_array().map(Vec::len),
        Some(super::MAX_FINAL_GLYPH_RECORDS)
    );
    /* 要求spanとlayout spanは別記録なので、上限超過した各1件を数える。 */
    assert_eq!(audit["finalGlyphDropped"], 2);
    Ok(())
}

#[test]
fn repeated_glyph_text_keeps_ambiguous_source_span_attribution() -> Result<(), String> {
    let svg = svg_with_repeated_glyph_text();
    let audit = audit_svg(&svg)?;
    let tree = root_tree(&audit)?;
    let spans = tree["textSpans"]
        .as_array()
        .ok_or("text span evidence is absent")?;
    assert_eq!(spans.len(), 2);
    for span in spans {
        assert_eq!(span["requestAssociation"], "ambiguous_text_match");
        assert!(span["requested"].is_null());
        assert_eq!(span["requestCandidateIndices"], serde_json::json!([0, 1]));
    }
    Ok(())
}

fn assert_glyph_cap(tree: &serde_json::Value) -> Result<(), String> {
    let spans = tree["textSpans"]
        .as_array()
        .ok_or("text span evidence is absent")?
        .as_slice();
    let recorded = spans
        .iter()
        .map(|span| span["glyphLayout"].as_array().map_or(0, Vec::len))
        .sum::<usize>();
    let total = spans
        .iter()
        .map(|span| {
            span["glyphCount"]
                .as_u64()
                .ok_or("glyph count evidence is absent")
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|count| count as usize)
        .sum::<usize>();
    assert_eq!(tree["glyphLayoutErrors"], serde_json::json!([]));
    assert!(total > super::MAX_FINAL_GLYPH_RECORDS);
    assert_eq!(recorded, super::MAX_FINAL_GLYPH_RECORDS);
    assert_eq!(tree["glyphLayoutDropped"], total - recorded);
    Ok(())
}

#[test]
fn glyph_layout_rejects_non_finite_values() -> Result<(), String> {
    let finite = [0.0; super::output::TRANSFORM_COMPONENTS];
    let mut invalid = Vec::new();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut transform = finite;
        transform[0] = value;
        invalid.push(("transform", 16.0, transform, finite));
        let mut outline = finite;
        outline[0] = value;
        invalid.push(("outlineTransform", 16.0, finite, outline));
        invalid.push(("fontSize", value, finite, finite));
    }
    for (field, font_size, transform, outline_transform) in invalid {
        match super::output::ensure_finite_layout(7, font_size, transform, outline_transform) {
            Ok(()) => return Err(format!("{field} was accepted")),
            Err(error) => assert!(error.contains("non-finite glyph layout")),
        }
    }
    Ok(())
}

#[test]
fn glyph_font_evidence_caps_unique_loaded_font_faces() -> Result<(), String> {
    let mut database = Database::new();
    let shared_font_bytes = Arc::new(FONT_BYTES.to_vec());
    let mut ids = Vec::new();
    for _ in 0..=super::MAX_FINAL_GLYPH_RECORDS {
        let loaded = database.load_font_source(Source::Binary(shared_font_bytes.clone()));
        if loaded.len() != 1 {
            return Err("bundled font did not load as exactly one real font face".into());
        }
        ids.push(loaded[0]);
    }

    let mut evidence = super::TreeEvidence::default();
    for id in &ids {
        super::record_glyph_font(*id, &mut evidence);
    }
    assert_eq!(evidence.faces.len(), super::MAX_FINAL_GLYPH_RECORDS);
    assert_eq!(evidence.dropped, 1);

    super::record_glyph_font(ids[0], &mut evidence);
    assert_eq!(evidence.faces.get(&ids[0]), Some(&2));
    assert_eq!(evidence.dropped, 1);
    Ok(())
}

fn svg_inputs() -> (String, String) {
    use base64::Engine;

    let embedded = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="24"><text x="1" y="18" font-family="Arial" font-size="16">nested</text></svg>"#;
    let encoded = base64::engine::general_purpose::STANDARD.encode(embedded);
    let first = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="28"><text x="1" y="20" font-family="Arial" font-size="16">first</text></svg>"#.to_owned();
    let second = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="32"><text x="1" y="20" font-family="Arial" font-size="16">second</text><image x="30" y="1" width="64" height="24" href="data:image/svg+xml;base64,{encoded}"/></svg>"#
    );
    (first, second)
}

fn assert_svg_tree_rows(
    audit: &serde_json::Value,
    first: &str,
    second: &str,
) -> Result<(), String> {
    let trees = audit["finalGlyphTrees"]
        .as_array()
        .ok_or_else(|| "final glyph tree evidence is absent".to_owned())?;
    assert_eq!(audit["finalGlyphPaintCalls"], 2);
    for (call, source) in [(1, first), (2, second)] {
        assert_tree_row(
            trees,
            call,
            "root",
            super::output::sha256(source.as_bytes()),
        );
        assert_glyph_layout(trees, call)?;
    }
    assert!(trees.iter().any(|tree| {
        tree["paintCall"] == 2
            && tree["treePath"]
                .as_str()
                .is_some_and(|path| path.contains("image-svg@root/"))
            && tree["sourceSvg"].is_null()
            && tree["resolvedSvg"].is_null()
            && tree["resolvedTextSvg"].is_null()
            && tree["resolvedTextSvgSha256"].is_null()
    }));
    Ok(())
}

fn assert_glyph_layout(trees: &[serde_json::Value], call: u64) -> Result<(), String> {
    let tree = trees
        .iter()
        .find(|tree| tree["paintCall"] == call && tree["treePath"] == "root")
        .ok_or("root glyph tree row is absent")?;
    assert_eq!(tree["glyphLayoutDropped"], 0);
    assert_eq!(tree["glyphLayoutErrors"], serde_json::json!([]));
    let spans = tree["textSpans"]
        .as_array()
        .ok_or("text spans are absent")?;
    for span in spans {
        assert!(span["textPath"].is_string());
        assert_finite_transform(span, "textAbsTransform")?;
        assert_span_glyphs(span)?;
    }
    Ok(())
}

fn assert_span_glyphs(span: &serde_json::Value) -> Result<(), String> {
    let glyphs = span["glyphLayout"]
        .as_array()
        .ok_or("glyph layout is absent")?;
    for glyph in glyphs {
        assert!(glyph["id"].is_u64());
        assert!(glyph["text"].is_string());
        assert!(glyph["fontId"].is_string());
        assert!(glyph["fontSize"].as_f64().is_some_and(f64::is_finite));
        assert_finite_transform(glyph, "transform")?;
        assert_finite_transform(glyph, "outlineTransform")?;
    }
    Ok(())
}

fn assert_finite_transform(glyph: &serde_json::Value, key: &str) -> Result<(), String> {
    let values = glyph[key].as_array().ok_or("transform is absent")?;
    assert_eq!(values.len(), super::output::TRANSFORM_COMPONENTS);
    assert!(
        values
            .iter()
            .all(|value| value.as_f64().is_some_and(f64::is_finite))
    );
    Ok(())
}

fn assert_tree_row(trees: &[serde_json::Value], call: u64, path: &str, sha: String) {
    assert!(trees.iter().any(|tree| {
        tree["paintCall"] == call
            && tree["treePath"] == path
            && tree["sourceSvgSha256"] == sha
            && tree["sourceSvg"]
                .as_str()
                .is_some_and(|source| super::output::sha256(source.as_bytes()) == sha)
            && tree["resolvedSvg"].is_string()
            && tree["resolvedSvg"].as_str().is_some_and(|resolved| {
                tree["resolvedSvgSha256"] == super::output::sha256(resolved.as_bytes())
            })
            && tree["resolvedTextSvg"].is_string()
            && tree["resolvedTextSvg"].as_str().is_some_and(|resolved| {
                tree["resolvedTextSvgSha256"] == super::output::sha256(resolved.as_bytes())
                    && resolved.contains("<text")
            })
            && tree["resolvedSvg"]
                .as_str()
                .is_some_and(|resolved| resolved.contains("<path"))
    }));
}
