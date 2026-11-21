use resvg::usvg::{self, fontdb};
use serde_json::{Value, json};
use sha2::Digest;

pub(super) fn span_record(
    requested: &usvg::TextSpan,
    span: &usvg::layout::Span,
    database: &fontdb::Database,
) -> Result<Value, String> {
    let glyphs = &span.positioned_glyphs;
    let faces = glyphs
        .iter()
        .map(|glyph| record(database, glyph.font))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        json!({"requested_weight": requested.font().weight(), "glyph_count": glyphs.len(),
        "variations": span.variations.iter().map(|axis| json!({"tag": String::from_utf8_lossy(&axis.tag), "value": axis.value})).collect::<Vec<_>>(),
        "faces": faces}),
    )
}

pub(super) fn record(database: &fontdb::Database, id: fontdb::ID) -> Result<Value, String> {
    let face = database.face(id).ok_or("layout font face missing")?;
    Ok(
        json!({"id": format!("{id:?}"), "post_script_name": face.post_script_name,
        "families": face.families.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        "face_weight": face.weight.0, "face_index": face.index,
        "source": source(&face.source), "data": data(database, id)?}),
    )
}

fn source(source: &fontdb::Source) -> Value {
    match source {
        fontdb::Source::File(path) => json!({"kind": "File", "path": path}),
        fontdb::Source::SharedFile(path, _) => json!({"kind": "SharedFile", "path": path}),
        fontdb::Source::Binary(_) => json!({"kind": "Binary"}),
    }
}

fn data(database: &fontdb::Database, id: fontdb::ID) -> Result<Value, String> {
    database.with_face_data(id, |bytes, index| {
        let digest = sha2::Sha256::digest(bytes);
        let parsed = rustybuzz::ttf_parser::Face::parse(bytes, index)
            .map_err(|_| "font face metadata parse failed")?;
        Ok(json!({"sha256": digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
            "bytes": bytes.len(), "index": index, "variationAxes": axes(&parsed)}))
    }).ok_or("layout glyph face bytes missing")?
}

fn axes(face: &rustybuzz::ttf_parser::Face<'_>) -> Vec<Value> {
    face.variation_axes()
        .into_iter()
        .map(|axis| {
            json!({"tag": format!("{:?}", axis.tag),
        "minimum": axis.min_value, "default": axis.def_value, "maximum": axis.max_value})
        })
        .collect()
}
