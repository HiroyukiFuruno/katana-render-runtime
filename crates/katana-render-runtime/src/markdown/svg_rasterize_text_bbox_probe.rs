use super::{
    canonical_svg_font_family, escape_xml_attribute, escape_xml_text, html_rasterizer_options, usvg,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use skrifa::{MetadataProvider, prelude::Size};
use std::{collections::HashSet, path::Path};
type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;
const FAMILY: &str = "Recursive Variable, arial, sans-serif";
#[path = "svg_rasterize_text_recursive_probe.rs"]
mod recursive_probe;
#[test]
#[ignore = "diagnostic only; requires a fresh absolute TMP output path"]
fn records_library_bbox_for_live_font_declaration() -> ProbeResult<()> {
    let output = std::env::var_os("KRR_TEXT_BBOX_PROBE_OUTPUT")
        .ok_or("missing KRR_TEXT_BBOX_PROBE_OUTPUT")?;
    let cases = [
        ("動物", 700),
        ("+int 年齢", 400),
        ("Animal", 700),
        ("+int age", 400),
    ];
    let rows = cases
        .into_iter()
        .map(|(text, weight)| observe(text, weight))
        .collect::<ProbeResult<Vec<_>>>()?;
    let record = json!({"schema": "usvg-text-bbox-probe-r1", "rows": rows,
        "coordinate_domain": "identity local SVG user units; not Live client pixels",
        "actual_live_font_proven": false, "score_accepted": false});
    write_fresh(Path::new(&output), &serde_json::to_vec_pretty(&record)?)?;
    Ok(())
}

fn observe(text: &str, weight: u16) -> ProbeResult<Value> {
    let svg = probe_svg(text, weight);
    let options = html_rasterizer_options(&svg);
    super::super::text_shaping::with_validated_tree_parse(|| {
        let tree = usvg::Tree::from_str(&svg, &options)?;
        let node = tree
            .node_by_id("krr-text-measure")
            .ok_or("text node missing")?;
        let bbox = node.abs_bounding_box();
        assert!(bbox.width().is_finite() && bbox.width() > 0.0);
        assert!(bbox.height().is_finite() && bbox.height() > 0.0);
        let usvg::Node::Text(text_node) = node else {
            return Err("expected text node".into());
        };
        let spans = observe_spans(text_node, tree.fontdb())?;
        let missing = missing_glyph_count(&spans)?;
        Ok(
            json!({"text": text, "missing_glyph_id_zero_count": missing, "requested_family": FAMILY, "requested_weight": weight,
            "size": 14.0, "italic": false, "letter_spacing": 0.0,
            "bbox": {"x": bbox.x(), "y": bbox.y(), "width": bbox.width(), "height": bbox.height()},
            "base_database_shared": std::sync::Arc::ptr_eq(&options.fontdb, tree.fontdb()),
            "spans": spans}),
        )
    })
}

fn probe_svg(text: &str, weight: u16) -> String {
    let family = escape_xml_attribute(&canonical_svg_font_family(FAMILY));
    let text = escape_xml_text(text);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="16384" height="56"><text id="krr-text-measure" x="0" y="28" font-family="{family}" font-size="14" font-weight="{weight}" font-style="normal" letter-spacing="0">{text}</text></svg>"#
    )
}

fn observe_spans(text: &usvg::Text, db: &usvg::fontdb::Database) -> ProbeResult<Vec<Value>> {
    text.layouted()
        .iter()
        .map(|span| observe_span(span, db))
        .collect()
}

fn observe_span(span: &usvg::layout::Span, db: &usvg::fontdb::Database) -> ProbeResult<Value> {
    let ids = span
        .positioned_glyphs
        .iter()
        .map(|glyph| glyph.font)
        .collect::<HashSet<_>>();
    let mut faces = ids
        .into_iter()
        .map(|id| observe_face(db, id, span))
        .collect::<ProbeResult<Vec<_>>>()?;
    faces.sort_by_key(|face| face.to_string());
    let glyphs = span
        .positioned_glyphs
        .iter()
        .map(|glyph| {
            json!({
                "glyph_id": glyph.id.0, "text": glyph.text, "font_id": format!("{:?}", glyph.font)
            })
        })
        .collect::<Vec<_>>();
    let missing = glyphs
        .iter()
        .filter(|g| g.get("glyph_id").and_then(Value::as_u64) == Some(0))
        .count();
    Ok(
        json!({"font_size": span.font_size.get(), "variations": variation_records(span),
        "optical_sizing": format!("{:?}", span.font_optical_sizing),
        "missing_glyph_id_zero_count": missing, "faces": faces, "glyphs": glyphs}),
    )
}

fn variation_records(span: &usvg::layout::Span) -> Vec<Value> {
    span.variations
        .iter()
        .map(|v| json!({"tag": String::from_utf8_lossy(&v.tag), "value": v.value}))
        .collect()
}
fn missing_glyph_count(spans: &[Value]) -> ProbeResult<u64> {
    spans.iter().try_fold(0, |count, span| {
        let missing = span["missing_glyph_id_zero_count"]
            .as_u64()
            .ok_or("missing glyph count unavailable")?;
        Ok(count + missing)
    })
}

fn observe_face(
    db: &usvg::fontdb::Database,
    id: usvg::fontdb::ID,
    span: &usvg::layout::Span,
) -> ProbeResult<Value> {
    let face = db.face(id).ok_or("face missing")?;
    let before = source_record(&face.source)?;
    let data = db
        .with_face_data(id, |bytes, index| data_record(bytes, index, span))
        .ok_or("face bytes unavailable")??;
    let after = source_record(&face.source)?;
    assert_eq!(before, after, "source changed during diagnostic face read");
    Ok(json!({"id": format!("{:?}", id), "index": face.index,
        "postscript_name": face.post_script_name, "face_weight": face.weight.0,
        "families": face.families.iter().map(|v| &v.0).collect::<Vec<_>>(),
        "source_before": before, "source_after": after, "data": data}))
}

fn data_record(bytes: &[u8], index: u32, span: &usvg::layout::Span) -> ProbeResult<Value> {
    let font = skrifa::FontRef::from_index(bytes, index)?;
    let location = font.axes().location(
        span.variations
            .iter()
            .map(|v| (skrifa::Tag::from_be_bytes(v.tag), v.value)),
    );
    let metrics = font.metrics(Size::new(span.font_size.get()), &location);
    Ok(json!({"sha256": sha256(bytes), "bytes": bytes.len(),
        "index": index, "normalized_coords": format!("{:?}", location.coords()),
        "font_metrics": {"ascent": metrics.ascent, "descent": metrics.descent,
            "leading": metrics.leading, "units_per_em": metrics.units_per_em}}))
}
pub(super) fn source_record(source: &usvg::fontdb::Source) -> ProbeResult<Value> {
    match source {
        usvg::fontdb::Source::Binary(_) => Ok(json!({"kind": "Binary"})),
        usvg::fontdb::Source::File(path) => file_record("File", path),
        usvg::fontdb::Source::SharedFile(path, _) => file_record("SharedFile", path),
    }
}

fn file_record(kind: &str, path: &Path) -> ProbeResult<Value> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?.duration_since(std::time::UNIX_EPOCH)?;
    #[cfg(unix)]
    let identity = {
        use std::os::unix::fs::MetadataExt;
        json!({"dev": metadata.dev(), "ino": metadata.ino()})
    };
    #[cfg(not(unix))]
    let identity = Value::Null;
    Ok(
        json!({"kind": kind, "path": path, "len": metadata.len(), "identity": identity,
        "modified_seconds": modified.as_secs(), "modified_nanos": modified.subsec_nanos()}),
    )
}

pub(super) use super::probe_test_support::write_fresh;
fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
