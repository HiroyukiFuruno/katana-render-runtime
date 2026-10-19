use super::{FontShape, HARFRUST_SCALE, face_cache, shape_font, shape_svg_run_metrics_with_face};
use resvg::usvg;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type DiagnosticRows = (String, Vec<Value>, String, String);
type GlyphMetricRecords = (Vec<Value>, Vec<Value>, Value);
const DIAGNOSTIC_FONT_SIZE: f32 = 16.0;
const GLYPH_POSITION_TOLERANCE: f32 = 0.001;

#[derive(Debug, Deserialize)]
struct Request {
    input: PathBuf,
    output: PathBuf,
    expected_source_sha256: String,
}

const CASES: [(&str, u16); 8] = [
    ("/", 700),
    ("Root", 400),
    ("Runtime", 400),
    ("V8", 400),
    ("DOM shim", 400),
    ("Output", 400),
    ("SVG", 400),
    ("Rasterize", 400),
];

#[test]
#[ignore = "diagnostic only; requires macOS system Trebuchet and a fresh request JSON"]
fn records_tree_trebuchet_harfrust_and_usvg_metrics() -> Result<()> {
    let request = read_request()?;
    let (source_sha, rows, source_svg, outline_svg) = read_rows(&request)?;
    write_report(&request, source_sha, rows, source_svg, outline_svg)
}

fn read_request() -> Result<Request> {
    let request_path = std::env::var_os("KRR_HTML_FONT_AUDIT_REQUEST")
        .ok_or("KRR_HTML_FONT_AUDIT_REQUEST is required")?;
    let request_path = PathBuf::from(request_path);
    if !request_path.is_absolute() {
        return Err("KRR_HTML_FONT_AUDIT_REQUEST must be absolute".into());
    }
    Ok(serde_json::from_slice(&std::fs::read(&request_path)?)?)
}

fn read_rows(request: &Request) -> Result<DiagnosticRows> {
    let input = std::fs::read(&request.input)?;
    let source_sha = digest_hex(&input);
    if source_sha != request.expected_source_sha256 {
        return Err(format!("input SHA mismatch: {source_sha}").into());
    }
    let svg = extract_svg(std::str::from_utf8(&input)?)?;
    let rows = CASES
        .into_iter()
        .map(|(text, weight)| observe_case(svg, text, weight))
        .collect::<Result<Vec<_>>>()?;
    let outline_svg = outline_svg(svg)?;
    Ok((source_sha, rows, svg.to_owned(), outline_svg))
}

fn write_report(
    request: &Request,
    source_sha: String,
    rows: Vec<Value>,
    source_svg: String,
    outline_svg: String,
) -> Result<()> {
    if request.output.exists() {
        return Err("diagnostic output must be fresh".into());
    }
    std::fs::create_dir(&request.output)?;
    write_fresh(&request.output.join("text.svg"), source_svg.as_bytes())?;
    write_fresh(&request.output.join("outline.svg"), outline_svg.as_bytes())?;
    let report = json!({
        "schema": "krr-tree-trebuchet-harfrust-usvg-metrics-v1",
        "input": request.input,
        "input_sha256": source_sha,
        "font_family": "Trebuchet MS",
        "rows": rows,
        "diagnostic_only": true,
        "formal_acceptance": false,
    });
    let report_path = request.output.join("report.json");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(report_path)?;
    serde_json::to_writer_pretty(file, &report)?;
    Ok(())
}

fn write_fresh(path: &Path, bytes: &[u8]) -> Result<()> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    std::io::Write::write_all(&mut std::io::BufWriter::new(file), bytes)?;
    Ok(())
}

fn outline_svg(source_svg: &str) -> Result<String> {
    let options = crate::markdown::svg_rasterize::font::html_rasterizer_options(source_svg);
    crate::markdown::svg_rasterize::text_shaping::with_validated_tree_parse(|| {
        let tree = usvg::Tree::from_str(source_svg, &options)?;
        let outline = tree.to_string(&usvg::WriteOptions {
            preserve_text: false,
            ..usvg::WriteOptions::default()
        });
        Ok(outline)
    })
}

fn extract_svg(html: &str) -> Result<&str> {
    let start = html.find("<svg ").ok_or("SVG start missing")?;
    let end = html[start..]
        .find("</svg>")
        .map(|offset| start + offset + "</svg>".len())
        .ok_or("SVG end missing")?;
    Ok(&html[start..end])
}

fn first_text_span(tree: &usvg::Tree) -> Result<&usvg::layout::Span> {
    let node = tree.node_by_id("run").ok_or("diagnostic text missing")?;
    let usvg::Node::Text(text_node) = node else {
        return Err("diagnostic node is not text".into());
    };
    text_node
        .layouted()
        .first()
        .ok_or_else(|| "diagnostic span missing".into())
}

fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn observe_case(source_svg: &str, text: &str, weight: u16) -> Result<Value> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="64"><text id="run" x="0" y="48" font-family="trebuchet ms,verdana,arial,sans-serif" font-size="16" font-weight="{weight}">{text}</text></svg>"#
    );
    let options = crate::markdown::svg_rasterize::font::html_rasterizer_options(source_svg);
    crate::markdown::svg_rasterize::text_shaping::with_validated_tree_parse(|| {
        let tree = usvg::Tree::from_str(&svg, &options)?;
        observe_tree_case(&tree, text, weight)
    })
}

fn observe_tree_case(tree: &usvg::Tree, text: &str, weight: u16) -> Result<Value> {
    let node = tree.node_by_id("run").ok_or("diagnostic text missing")?;
    let usvg::Node::Text(text_node) = node else {
        return Err("diagnostic node is not text".into());
    };
    let span = text_node
        .layouted()
        .first()
        .ok_or("diagnostic span missing")?;
    let face_id = span
        .positioned_glyphs
        .first()
        .ok_or("diagnostic glyph missing")?
        .font;
    let (harfrust, usvg_glyphs, metrics) = collect_case_metrics(tree, span, face_id, text, weight)?;
    let face = tree.fontdb().face(face_id).map(face_record);
    Ok(json!({
        "text": text,
        "weight": weight,
        "face_id": format!("{:?}", face_id),
        "face": face,
        "harfrust": harfrust,
        "usvg": usvg_glyphs,
        "usvg_metrics": metrics,
        "glyph_count_equal": harfrust.len() == span.positioned_glyphs.len(),
    }))
}

fn collect_case_metrics(
    tree: &usvg::Tree,
    span: &usvg::layout::Span,
    face_id: usvg::fontdb::ID,
    text: &str,
    weight: u16,
) -> Result<GlyphMetricRecords> {
    let face = face_cache::font_for_face(tree.fontdb(), face_id)
        .ok_or("Trebuchet face bytes unavailable")?;
    let shape = FontShape::new(DIAGNOSTIC_FONT_SIZE, weight, false);
    let buffer = shape_font(&face, text, shape, None).ok_or("Harfrust shaping failed")?;
    let harfrust = harfrust_records(&buffer);
    let usvg = usvg_records(span);
    assert_glyph_positions(&harfrust, &span.positioned_glyphs, text)?;
    Ok((
        harfrust,
        usvg,
        metrics_record(tree.fontdb(), face_id, text, shape)?,
    ))
}

fn face_record(face: &usvg::fontdb::FaceInfo) -> Value {
    json!({
        "families": face.families.iter().map(|(family, _)| family).collect::<Vec<_>>(),
        "weight": face.weight.0,
    })
}

fn harfrust_records(buffer: &harfrust::Buffer) -> Vec<Value> {
    buffer
        .glyph_infos()
        .iter()
        .zip(buffer.glyph_positions())
        .map(|(info, position)| {
            json!({
                "glyph_id": info.glyph_id,
                "x_advance": position.x_advance as f32 / HARFRUST_SCALE,
                "y_advance": position.y_advance as f32 / HARFRUST_SCALE,
                "x_offset": position.x_offset as f32 / HARFRUST_SCALE,
                "y_offset": position.y_offset as f32 / HARFRUST_SCALE,
            })
        })
        .collect()
}

fn usvg_records(span: &usvg::layout::Span) -> Vec<Value> {
    span.positioned_glyphs
        .iter()
        .map(|glyph| {
            let transform = glyph.transform();
            json!({
                "glyph_id": glyph.id.0,
                "font_id": format!("{:?}", glyph.font),
                "text": glyph.text,
                "tx": transform.tx,
                "ty": transform.ty,
            })
        })
        .collect()
}

fn metrics_record(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    shape: FontShape,
) -> Result<Value> {
    let (metrics, ink) = shape_svg_run_metrics_with_face(database, face_id, text, shape, true)
        .ok_or("usvg run metrics failed")?;
    Ok(json!({
        "advance": metrics.advance,
        "ascent": metrics.ascent,
        "descent": metrics.descent,
        "layout_ascent": metrics.layout_ascent,
        "ink_bounds": metrics.ink_bounds,
        "ink_diagnostic": ink.map(|value| json!({
            "bounds": value.bounds,
            "glyphs_without_extents": value.glyphs_without_extents,
        })),
    }))
}

fn assert_glyph_positions(
    harfrust: &[Value],
    usvg: &[usvg::layout::PositionedGlyph],
    text: &str,
) -> Result<()> {
    if harfrust.len() != usvg.len() {
        return Err(format!("glyph count mismatch for {text}").into());
    }
    let mut x = 0.0_f32;
    for (harfrust, usvg) in harfrust.iter().zip(usvg) {
        let glyph_id = harfrust["glyph_id"].as_u64().ok_or("glyph ID missing")?;
        let advance = harfrust["x_advance"].as_f64().ok_or("advance missing")? as f32;
        let offset = harfrust["x_offset"].as_f64().ok_or("offset missing")? as f32;
        let positioned_x = x + offset;
        if glyph_id != u64::from(usvg.id.0)
            || (positioned_x - usvg.transform().tx).abs() > GLYPH_POSITION_TOLERANCE
        {
            return Err(format!(
                "glyph position mismatch for {text}: id {glyph_id}/{} x {positioned_x}/{}",
                usvg.id.0,
                usvg.transform().tx
            )
            .into());
        }
        x += advance;
    }
    Ok(())
}

#[path = "svg_rasterize_text_coretext_diagnostic_tests.rs"]
mod coretext_diagnostic;
