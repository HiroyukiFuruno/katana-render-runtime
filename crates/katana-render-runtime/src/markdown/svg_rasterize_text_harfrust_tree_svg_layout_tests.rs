use super::{Result, Value, digest_hex, extract_svg, json};
use resvg::usvg::fontdb::{Database, ID, Source};
use resvg::usvg::{self, Group, Node, Tree};
use serde_json::Value as JsonValue;

pub(super) fn verify_input_sha(
    path: &std::path::Path,
    input: &[u8],
    expected: &str,
) -> Result<String> {
    let actual = digest_hex(input);
    if actual != expected {
        return Err(format!("input SHA mismatch for {}: {actual}", path.display()).into());
    }
    Ok(actual)
}

pub(super) fn require_fresh_output(path: &std::path::Path) -> Result<()> {
    if path.exists() {
        return Err("diagnostic output must be fresh".into());
    }
    Ok(())
}

pub(super) fn write_report_json(
    request: &super::Request,
    source_sha: String,
    rows: Vec<Value>,
    source_layout: Value,
) -> Result<()> {
    let report = json!({
        "schema": "krr-tree-trebuchet-harfrust-usvg-layout-v2",
        "input": request.input,
        "input_sha256": source_sha,
        "input_svg_layout": source_layout,
        "font_family": "Trebuchet MS",
        "rows": rows,
        "diagnostic_only": true,
        "formal_acceptance": false,
    });
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(request.output.join("report.json"))?;
    serde_json::to_writer_pretty(file, &report)?;
    Ok(())
}

pub(super) fn observe_source_layout(
    input_path: &std::path::Path,
    expected_input_sha: &str,
    source_svg: &str,
) -> Result<Value> {
    let input = std::fs::read(input_path)?;
    let input_sha = verify_input_sha(input_path, &input, expected_input_sha)?;
    let extracted = extract_svg(std::str::from_utf8(&input)?)?;
    if extracted != source_svg {
        return Err("extracted source SVG changed during diagnostic".into());
    }
    let source_svg_sha = digest_hex(source_svg.as_bytes());
    let candidate_options =
        crate::markdown::svg_rasterize::font::html_rasterizer_options(source_svg);
    let (candidate_texts, candidate_faces) = parse_candidate(source_svg, &candidate_options)?;
    Ok(json!({
        "schema": "krr-usvg-candidate-positioned-glyphs-v2",
        "input_path": input_path,
        "input_sha256": input_sha,
        "source_svg_sha256": source_svg_sha,
        "svg_bytes_parsed_without_transformation": true,
        "coordinate_domain": "usvg SVG user units; not Chrome CSSOM or paint pixels",
        "observation_scope": "candidate usvg layout only; not native final paint",
        "krr_candidate_font_options": "html_rasterizer_options(source_svg)",
        "krr_candidate_texts": candidate_texts,
        "glyph_referenced_faces": candidate_faces,
        "character_to_glyph_mapping": "not-proven; PositionedGlyph.text has no source character index",
    }))
}

fn parse_candidate(svg: &str, options: &usvg::Options) -> Result<(Vec<JsonValue>, Vec<JsonValue>)> {
    crate::markdown::svg_rasterize::text_shaping::with_validated_tree_parse(|| {
        let tree = Tree::from_str(svg, options)?;
        let mut records = Vec::new();
        collect_texts(tree.root(), "root", &mut records);
        let mut ids = Vec::new();
        collect_font_ids(tree.root(), &mut ids);
        let faces = ids
            .into_iter()
            .map(|id| face_record(tree.fontdb(), id))
            .collect::<Result<Vec<_>>>()?;
        Ok((records, faces))
    })
}

fn collect_font_ids(group: &Group, ids: &mut Vec<ID>) {
    for node in group.children() {
        match node {
            Node::Group(group) => collect_font_ids(group, ids),
            Node::Text(text) => add_text_font_ids(text, ids),
            Node::Path(_) | Node::Image(_) => {}
        }
    }
}

fn add_text_font_ids(text: &usvg::Text, ids: &mut Vec<ID>) {
    for glyph in text
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
    {
        if !ids.contains(&glyph.font) {
            ids.push(glyph.font);
        }
    }
}

fn face_record(database: &Database, id: ID) -> Result<JsonValue> {
    let face = database.face(id).ok_or("glyph fontdb face is missing")?;
    let source = match &face.source {
        Source::File(path) => path.display().to_string(),
        Source::SharedFile(path, _) => path.display().to_string(),
        Source::Binary(_) => "binary font data".to_string(),
    };
    let (bytes_sha256, data_index) = database
        .with_face_data(id, |bytes, index| (digest_hex(bytes), index))
        .ok_or("glyph fontdb face data is unavailable")?;
    Ok(json!({
        "fontdb_id": format!("{id:?}"),
        "source": source,
        "post_script_name": face.post_script_name,
        "families": face.families.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        "weight": face.weight.0,
        "face_index": face.index,
        "face_data_index": data_index,
        "face_data_sha256": bytes_sha256,
    }))
}

fn collect_texts(group: &Group, path: &str, records: &mut Vec<JsonValue>) {
    for (index, node) in group.children().iter().enumerate() {
        let node_path = format!("{path}/{index}");
        match node {
            Node::Group(group) => collect_texts(group, &node_path, records),
            Node::Text(text) => records.push(text_record(text, &node_path)),
            Node::Path(_) | Node::Image(_) => {}
        }
    }
}

fn text_record(text: &usvg::Text, node_path: &str) -> JsonValue {
    let source_text = text
        .chunks()
        .iter()
        .map(|chunk| chunk.text())
        .collect::<String>();
    let spans = text
        .layouted()
        .iter()
        .enumerate()
        .map(|(index, span)| span_record(index, span))
        .collect::<Vec<_>>();
    json!({
        "node_path": node_path,
        "id": text.id(),
        "text": source_text,
        "text_transform": transform_record(text.abs_transform()),
        "bounding_box": bounding_box_record(text),
        "chunks": chunk_records(text),
        "layouted_spans": spans,
    })
}

fn bounding_box_record(text: &usvg::Text) -> JsonValue {
    let bounds = text.bounding_box();
    json!({"x":bounds.x(), "y":bounds.y(), "width":bounds.width(), "height":bounds.height()})
}

fn chunk_records(text: &usvg::Text) -> Vec<JsonValue> {
    text.chunks().iter().enumerate().map(chunk_record).collect()
}

fn chunk_record((index, chunk): (usize, &usvg::TextChunk)) -> JsonValue {
    json!({
        "index": index,
        "text": chunk.text(),
        "x": chunk.x(),
        "y": chunk.y(),
        "anchor": format!("{:?}", chunk.anchor()),
        "flow": format!("{:?}", chunk.text_flow()),
    })
}

fn span_record(index: usize, span: &usvg::layout::Span) -> JsonValue {
    let glyphs = span
        .positioned_glyphs
        .iter()
        .map(|glyph| {
            json!({
                "glyph_id": glyph.id.0,
                "text": glyph.text,
                "font_id": format!("{:?}", glyph.font),
                "font_size": glyph.font_size(),
                "transform": transform_record(glyph.transform()),
            })
        })
        .collect::<Vec<_>>();
    json!({"index": index, "font_size": span.font_size.get(), "glyphs": glyphs})
}

fn transform_record(transform: usvg::Transform) -> JsonValue {
    json!({
        "sx": transform.sx,
        "sy": transform.sy,
        "kx": transform.kx,
        "ky": transform.ky,
        "tx": transform.tx,
        "ty": transform.ty,
    })
}

#[cfg(test)]
mod tests {
    use super::{JsonValue, Node, Tree, parse_candidate, require_fresh_output, verify_input_sha};
    use resvg::usvg;
    use std::sync::Arc;

    fn transform_is_finite(transform: &JsonValue) -> bool {
        ["sx", "sy", "kx", "ky", "tx", "ty"]
            .iter()
            .all(|key| transform[*key].as_f64().is_some_and(f64::is_finite))
    }

    #[test]
    fn records_bundled_font_text_and_finite_glyph_transforms()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut database = usvg::fontdb::Database::new();
        database
            .load_font_data(crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES.to_vec());
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="probe" x="4" y="24" font-family="Noto Sans" font-size="18">AV</text></svg>"#;
        let options = usvg::Options {
            fontdb: Arc::new(database),
            ..usvg::Options::default()
        };
        let tree = Tree::from_str(svg, &options)?;
        let (records, faces) = parse_candidate(svg, &options)?;
        let Node::Text(text) = tree.root().children().first().ok_or("text missing")? else {
            return Err("expected text node".into());
        };
        assert_layout_record(&records, &faces)?;
        assert_eq!(text.id(), "probe");
        Ok(())
    }

    fn assert_layout_record(
        records: &[JsonValue],
        faces: &[JsonValue],
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["id"], "probe");
        assert_eq!(records[0]["text"], "AV");
        assert_eq!(faces.len(), 1);
        assert_eq!(faces[0]["post_script_name"], "NotoSans-Regular");
        assert!(faces[0]["face_data_sha256"].as_str().is_some());
        assert!(transform_is_finite(&records[0]["text_transform"]));
        assert_finite_recorded_glyph(&records[0])
    }

    fn assert_finite_recorded_glyph(record: &JsonValue) -> Result<(), Box<dyn std::error::Error>> {
        let glyphs = record["layouted_spans"][0]["glyphs"]
            .as_array()
            .ok_or("glyph list missing")?;
        let glyph = glyphs.first().ok_or("positioned glyph missing")?;
        assert!(glyph["glyph_id"].as_u64().is_some());
        assert!(glyph["text"].as_str().is_some());
        assert!(glyph["font_id"].as_str().is_some());
        assert!(transform_is_finite(&glyph["transform"]));
        Ok(())
    }

    #[test]
    fn rejects_changed_input_hash_and_existing_output() {
        assert!(verify_input_sha(std::path::Path::new("fixture.svg"), b"<svg/>", "wrong").is_err());
        assert!(require_fresh_output(std::path::Path::new(".")).is_err());
    }
}
