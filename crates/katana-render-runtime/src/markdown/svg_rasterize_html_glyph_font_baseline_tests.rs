use super::super::super::super::super::composite::TextRasterizer;
use super::super::super::super::HtmlGlyphRasterizer;
use image::ImageEncoder;
use resvg::usvg::{self, Node, Options, Tree};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tiny_skia::{Color, Pixmap, Transform};

const WIDTH: u32 = 160;
const HEIGHT: u32 = 80;
const HELVETICA: &str = "Helvetica";
const TREBUCHET: &str = "Trebuchet MS";
const TEXT: &str = "API Application";
const BASELINE_Y: f32 = 40.0;
const REGULAR_WEIGHT: u16 = 400;
const BOLD_WEIGHT: u16 = 700;
const QUARTER_PIXEL: f32 = 0.25;
const HALF_PIXEL: f32 = 0.5;
const THREE_QUARTER_PIXEL: f32 = 0.75;
const TREBUCHET_WEIGHTS: [u16; 2] = [REGULAR_WEIGHT, BOLD_WEIGHT];
const TREBUCHET_PHASES: [(&str, f32); 4] = [
    ("40", BASELINE_Y),
    ("40_25", BASELINE_Y + QUARTER_PIXEL),
    ("40_5", BASELINE_Y + HALF_PIXEL),
    ("40_75", BASELINE_Y + THREE_QUARTER_PIXEL),
];
const RGBA_CHANNELS: usize = 4;
const RGB_CHANNELS: usize = 3;
const WHITE_RGB: [u8; RGB_CHANNELS] = [u8::MAX; RGB_CHANNELS];

#[derive(Clone)]
struct BaselineCase {
    name: String,
    family: &'static str,
    dominant: &'static str,
    alignment: &'static str,
    weight: u16,
    y: f32,
}

struct RenderedCase {
    source: String,
    png: Vec<u8>,
    metadata: Value,
}

const HELVETICA_CASES: [(&str, &str, &str, u16); 6] = [
    ("alphabetic-400", "auto", "baseline", REGULAR_WEIGHT),
    ("alphabetic-700", "auto", "baseline", BOLD_WEIGHT),
    (
        "middle-mathematical-400",
        "middle",
        "mathematical",
        REGULAR_WEIGHT,
    ),
    (
        "middle-mathematical-700",
        "middle",
        "mathematical",
        BOLD_WEIGHT,
    ),
    ("central-400", "central", "central", REGULAR_WEIGHT),
    ("central-700", "central", "central", BOLD_WEIGHT),
];

fn cases() -> Vec<BaselineCase> {
    let mut cases = HELVETICA_CASES
        .into_iter()
        .map(|(name, dominant, alignment, weight)| BaselineCase {
            name: name.to_owned(),
            family: HELVETICA,
            dominant,
            alignment,
            weight,
            y: BASELINE_Y,
        })
        .collect::<Vec<_>>();
    append_trebuchet_cases(&mut cases);
    cases
}

fn append_trebuchet_cases(cases: &mut Vec<BaselineCase>) {
    for (label, y) in TREBUCHET_PHASES {
        for weight in TREBUCHET_WEIGHTS {
            cases.push(BaselineCase {
                name: format!("trebuchet-y{label}-{weight}"),
                family: TREBUCHET,
                dominant: "auto",
                alignment: "baseline",
                weight,
                y,
            });
        }
    }
}

#[test]
fn baseline_cases_render_with_finite_glyph_metadata() -> Result<(), String> {
    for case in cases() {
        let rendered = render_case(&case)?;
        if rendered.png.is_empty() {
            return Err(format!("{} produced an empty PNG", case.name));
        }
    }
    Ok(())
}

#[test]
#[ignore = "diagnostic only; requires a fresh output directory"]
fn baseline_cases_write_png_and_metadata() -> Result<(), String> {
    let output = diagnostic_output()?;
    std::fs::create_dir_all(&output).map_err(|error| format!("create output: {error}"))?;
    let mut rows = Vec::new();
    for case in cases() {
        let rendered = render_case(&case)?;
        write_fresh(&output.join(format!("{}.png", case.name)), &rendered.png)?;
        write_fresh(
            &output.join(format!("{}.svg", case.name)),
            rendered.source.as_bytes(),
        )?;
        rows.push(rendered.metadata);
    }
    write_fresh(
        &output.join("metadata.json"),
        serde_json::to_vec_pretty(&json!({ "cases": rows }))
            .map_err(|error| error.to_string())?
            .as_slice(),
    )
}

fn diagnostic_output() -> Result<PathBuf, String> {
    let value = std::env::var_os("KRR_HTML_GLYPH_BASELINE_DIAGNOSTIC_OUTPUT")
        .map(PathBuf::from)
        .ok_or("KRR_HTML_GLYPH_BASELINE_DIAGNOSTIC_OUTPUT is required")?;
    value
        .is_absolute()
        .then_some(value)
        .ok_or_else(|| "KRR_HTML_GLYPH_BASELINE_DIAGNOSTIC_OUTPUT must be absolute".to_owned())
}

fn render_case(case: &BaselineCase) -> Result<RenderedCase, String> {
    let source = source_svg(case);
    let database =
        super::super::super::super::super::font::html_font_db_for_text(case.family, TEXT);
    let options = Options {
        fontdb: database.clone(),
        ..Options::default()
    };
    let tree = Tree::from_str(&source, &options).map_err(|error| error.to_string())?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err(format!("{} text node missing", case.name));
    };
    let rasterizer = HtmlGlyphRasterizer::new(&database);
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).ok_or("pixmap allocation failed")?;
    pixmap.fill(Color::WHITE);
    let mut target = pixmap.as_mut();
    if !rasterizer.render(text, Transform::identity(), &mut target)? {
        return Err(format!("{} glyph path unsupported", case.name));
    }
    let metadata = metadata(case, &source, &rasterizer, text)?;
    if !has_nonwhite_pixel(&pixmap) {
        return Err(format!("{} produced no visible pixels", case.name));
    }
    Ok(RenderedCase {
        source,
        png: encode_png(&pixmap)?,
        metadata,
    })
}

fn source_svg(case: &BaselineCase) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}">
<rect width="100%" height="100%" fill="white"/>
<text id="run" x="8" y="{}" font-family="{}" font-size="16" font-weight="{}" fill="black" dominant-baseline="{}"><tspan alignment-baseline="{}">{TEXT}</tspan></text>
</svg>"#,
        case.y, case.family, case.weight, case.dominant, case.alignment
    )
}

fn metadata(
    case: &BaselineCase,
    source: &str,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    text: &usvg::Text,
) -> Result<Value, String> {
    let glyphs = text
        .layouted()
        .iter()
        .flat_map(|span| span.positioned_glyphs.iter())
        .collect::<Vec<_>>();
    if glyphs.is_empty() || !valid_glyphs(&glyphs) {
        return Err(format!("{} has invalid glyph placement", case.name));
    }
    let font_id = glyphs[0].font;
    if glyphs.iter().any(|glyph| glyph.font != font_id) {
        return Err(format!("{} resolved multiple font faces", case.name));
    }
    Ok(json!({
        "case": case.name.as_str(), "family": case.family, "y": case.y,
        "sourceSvgSha256": sha256(source.as_bytes()),
        "font": font_metadata(rasterizer, font_id, case.family)?,
        "flags": {"subpixel": true, "linearMetrics": true, "baselineSnap": true, "hinting": "normal", "edging": "subpixelAntiAlias"},
        "glyphCount": glyphs.len(),
        "glyphs": glyph_metadata(&glyphs),
    }))
}

fn valid_glyphs(glyphs: &[&usvg::layout::PositionedGlyph]) -> bool {
    glyphs.iter().all(|glyph| {
        let transform = glyph.transform();
        transform.is_finite()
            && glyph.font_size().is_finite()
            && glyph.font_size() > 0.0
            && (0.0..=WIDTH as f32).contains(&transform.tx)
            && (0.0..=HEIGHT as f32).contains(&transform.ty)
    })
}

fn glyph_metadata(glyphs: &[&usvg::layout::PositionedGlyph]) -> Vec<Value> {
    glyphs
        .iter()
        .map(|glyph| {
            let transform = glyph.transform();
            json!({"id": glyph.id.0, "fontSize": glyph.font_size(), "transform": {
                "sx": transform.sx, "ky": transform.ky, "kx": transform.kx,
                "sy": transform.sy, "tx": transform.tx, "ty": transform.ty
            }})
        })
        .collect()
}

fn font_metadata(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    font_id: usvg::fontdb::ID,
    family: &str,
) -> Result<Value, String> {
    let (path, source_index) = rasterizer
        .face_file(font_id)?
        .ok_or("baseline font is not file-backed")?;
    let (cached, data, cached_index) = super::super::cached_source(rasterizer, font_id)?;
    let source = std::fs::read(&path).map_err(|error| error.to_string())?;
    Ok(json!({
        "family": family, "fontId": format!("{font_id:?}"),
        "path": path, "sourceIndex": source_index, "sourceSha256": sha256(&source),
        "cachedIndex": cached_index, "cachedSha256": sha256(data.as_bytes()),
        "postScriptName": cached.post_script_name(), "unitsPerEm": cached.units_per_em(),
    }))
}

fn has_nonwhite_pixel(pixmap: &Pixmap) -> bool {
    pixmap
        .data()
        .chunks_exact(RGBA_CHANNELS)
        .any(|pixel| pixel[..RGB_CHANNELS] != WHITE_RGB)
}

fn encode_png(pixmap: &Pixmap) -> Result<Vec<u8>, String> {
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            pixmap.data(),
            WIDTH,
            HEIGHT,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|error| error.to_string())?;
    Ok(png)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_fresh(path: &Path, bytes: &[u8]) -> Result<(), String> {
    crate::markdown::svg_rasterize::probe_test_support::write_fresh_file(path, bytes)
        .map_err(|error| error.to_string())
}
