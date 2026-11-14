use super::super::extract_svg;
use super::{
    DIAGNOSTIC_FONT_SIZE, FontShape, Result, case_font, digest_hex, harfrust_records,
    prepare_diagnostic, shaped_case, write_fresh,
};
use core_foundation::attributed_string::CFMutableAttributedString;
use core_foundation::base::{CFRange, TCFType};
use core_foundation::string::CFString;
use core_text::font::{CTFont, new_from_descriptor};
use core_text::font_manager::create_font_descriptor;
use core_text::line::CTLine;
use core_text::string_attributes::kCTFontAttributeName;
use serde_json::{Value, json};
use std::path::Path;

type HiraginoCaseData = (harfrust::Buffer, Vec<u8>, u32, String, String);
const HIRAGINO_CASES: [&str; 3] = ["データベース", "ストレージ", "サーバー"];
const HIRAGINO_FAMILY: &str = "Hiragino Sans";
const HIRAGINO_FONT_WEIGHT: u16 = 400;
const HIRAGINO_FACE_SHA256: &str =
    "ff92033a300f55aff2f45f57479a560af2c5467973b425d3f724f870af07f83e";
const HIRAGINO_POSTSCRIPT_NAME: &str = "HiraginoSans-W3";

#[test]
#[ignore = "diagnostic only; requires macOS Hiragino W3 and a fresh request JSON"]
fn records_hiragino_coretext_and_harfrust_shaping() -> Result<()> {
    let (request, input) = prepare_diagnostic()?;
    let source_svg = extract_svg(std::str::from_utf8(&input)?)?;
    let rows = HIRAGINO_CASES
        .into_iter()
        .enumerate()
        .map(|(index, text)| hiragino_case(&request.output, index, source_svg, text))
        .collect::<Result<Vec<_>>>()?;
    write_fresh(&request.output.join("input.html"), &input)?;
    write_hiragino_report(&request.output, &request.input, &input, rows)
}

fn write_hiragino_report(
    output: &Path,
    input: &Path,
    bytes: &[u8],
    rows: Vec<Value>,
) -> Result<()> {
    let report = json!({
        "schema": "krr-hiragino-coretext-harfrust-shaping-diagnostic-v1",
        "input": input,
        "input_sha256": digest_hex(bytes),
        "font_family": HIRAGINO_FAMILY,
        "font_face_sha256": HIRAGINO_FACE_SHA256,
        "font_postscript_name": HIRAGINO_POSTSCRIPT_NAME,
        "font_size": DIAGNOSTIC_FONT_SIZE,
        "font_weight": HIRAGINO_FONT_WEIGHT,
        "rows": rows,
        "diagnostic_only": true,
        "formal_acceptance": false,
    });
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("report.json"))?;
    serde_json::to_writer_pretty(file, &report)?;
    Ok(())
}

fn hiragino_case(output: &Path, index: usize, source_svg: &str, text: &str) -> Result<Value> {
    let row = hiragino_row(source_svg, text)?;
    let path = output.join(format!("case-{index:02}.json"));
    write_fresh(&path, serde_json::to_vec_pretty(&row)?.as_slice())?;
    Ok(row)
}

fn hiragino_row(source_svg: &str, text: &str) -> Result<Value> {
    let (harfrust, bytes, face_index, source, postscript_name) =
        hiragino_font_data(source_svg, text)?;
    Ok(json!({
        "text": text,
        "font_source": source,
        "face_index": face_index,
        "postscript_name": postscript_name,
        "font_sha256": digest_hex(&bytes),
        "harfrust": harfrust_records(&harfrust),
        "harfrust_variations": {"wght": f32::from(HIRAGINO_FONT_WEIGHT), "opsz": DIAGNOSTIC_FONT_SIZE, "ital": 0.0},
        "coretext": coretext_line_records(&bytes, text)?,
    }))
}

fn hiragino_font_data(source_svg: &str, text: &str) -> Result<HiraginoCaseData> {
    let svg = hiragino_svg(text);
    let options = crate::markdown::svg_rasterize::font::html_rasterizer_options(source_svg);
    crate::markdown::svg_rasterize::text_shaping::with_validated_tree_parse(|| {
        let (tree, face_id, _) = shaped_case(&svg, &options)?;
        let shape = FontShape::new(DIAGNOSTIC_FONT_SIZE, HIRAGINO_FONT_WEIGHT, false);
        let (harfrust, bytes, face_index, source) = case_font(&tree, face_id, text, shape)?;
        let postscript_name = tree
            .fontdb()
            .face(face_id)
            .ok_or("Hiragino face metadata missing")?
            .post_script_name
            .clone();
        require_hiragino_face(&bytes, face_index, &postscript_name)?;
        Ok((harfrust, bytes, face_index, source, postscript_name))
    })
}

fn hiragino_svg(text: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="64"><text id="run" x="0" y="48" font-family="{HIRAGINO_FAMILY}" font-size="16" font-weight="400">{text}</text></svg>"#
    )
}

fn require_hiragino_face(bytes: &[u8], face_index: u32, postscript_name: &str) -> Result<()> {
    if face_index != 0
        || digest_hex(bytes) != HIRAGINO_FACE_SHA256
        || postscript_name != HIRAGINO_POSTSCRIPT_NAME
    {
        return Err(format!(
            "unexpected Hiragino face: sha {} index {face_index} postscript {postscript_name}",
            digest_hex(bytes)
        )
        .into());
    }
    Ok(())
}

fn coretext_line_records(font_bytes: &[u8], text: &str) -> Result<Value> {
    let font = coretext_font(font_bytes)?;
    let string = coretext_attributed_string(&font, text);
    let line = CTLine::new_with_attributed_string(string.as_concrete_TypeRef());
    Ok(coretext_line_value(&line))
}

fn coretext_font(font_bytes: &[u8]) -> Result<CTFont> {
    let descriptor = create_font_descriptor(font_bytes)
        .map_err(|_| "CoreText font descriptor creation failed")?;
    let font = new_from_descriptor(&descriptor, f64::from(DIAGNOSTIC_FONT_SIZE));
    if font.postscript_name() != HIRAGINO_POSTSCRIPT_NAME {
        return Err(format!(
            "CoreText selected unexpected face: {}",
            font.postscript_name()
        )
        .into());
    }
    Ok(font)
}

fn coretext_attributed_string(font: &CTFont, text: &str) -> CFMutableAttributedString {
    let mut string = CFMutableAttributedString::new();
    string.replace_str(&CFString::new(text), CFRange::init(0, 0));
    let length = string.char_len();
    unsafe {
        string.set_attribute(CFRange::init(0, length), kCTFontAttributeName, font);
    }
    string
}

fn coretext_line_value(line: &CTLine) -> Value {
    let bounds = line.get_typographic_bounds();
    let runs = line
        .glyph_runs()
        .iter()
        .map(|run| coretext_run_record(&run))
        .collect::<Vec<_>>();
    json!({
        "postscript_name": HIRAGINO_POSTSCRIPT_NAME,
        "width": bounds.width,
        "ascent": bounds.ascent,
        "descent": bounds.descent,
        "leading": bounds.leading,
        "runs": runs,
    })
}

fn coretext_run_record(run: &core_text::run::CTRun) -> Value {
    let positions = run.positions();
    let glyphs = run.glyphs();
    let indices = run.string_indices();
    let bounds = run.get_typographic_bounds();
    json!({
        "glyphs": glyphs.to_vec(),
        "positions": positions.iter().map(|point| [point.x, point.y]).collect::<Vec<_>>(),
        "string_indices": indices.to_vec(),
        "width": bounds.width,
        "ascent": bounds.ascent,
        "descent": bounds.descent,
        "leading": bounds.leading,
    })
}
