use super::HtmlGlyphRasterizer;
use super::{ALPHA_CHANNEL, PreparedSpan, RGBA_CHANNELS, paint};
use resvg::usvg::{
    Node, Options, Tree,
    fontdb::{Database, ID, Weight},
};
use skia_safe::{AlphaType, ColorType, Font, FontMgr, ImageInfo, surfaces};
use std::sync::Arc;

#[path = "svg_rasterize_html_glyph_coretext_backend_tests.rs"]
mod coretext_backend_tests;
#[path = "svg_rasterize_coretext_raster_candidate_tests.rs"]
mod coretext_raster_candidate_tests;
#[path = "svg_rasterize_html_glyph_file_cache_probe_tests.rs"]
mod file_cache_probe_tests;
#[path = "svg_rasterize_html_glyph_typeface_helvetica_tests.rs"]
mod helvetica_tests;
#[path = "svg_rasterize_html_glyph_system_cache_probe_tests.rs"]
mod system_cache_probe_tests;

const FAMILY: &str = "Trebuchet MS";
const SAMPLE: &str = "AV";
const SAMPLE_FONT_SIZE: f32 = 16.0;
const PIXEL_WIDTH: i32 = 128;
const PIXEL_HEIGHT: i32 = 64;
const OPAQUE_ALPHA: u8 = 255;

#[test]
fn file_and_coretext_typefaces_are_compared_for_regular_and_bold() -> Result<(), String> {
    let database = super::super::font::html_font_db_for_text(FAMILY, SAMPLE);
    let rasterizer = HtmlGlyphRasterizer::new(&database);
    for weight in [Weight::NORMAL, Weight::BOLD] {
        compare_family_weight(&database, &rasterizer, FAMILY, weight)?;
    }
    Ok(())
}

pub(super) fn compare_family_weight(
    database: &Database,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    family: &str,
    weight: Weight,
) -> Result<(), String> {
    let id = find_face(database, family, weight)?;
    let cached = rasterizer
        .ensure_face(id)?
        .ok_or("file typeface unavailable")?;
    let typeface = cached.typeface.as_skia()?;
    let system = FontMgr::default()
        .legacy_make_typeface(Some(family), typeface.font_style())
        .ok_or("CoreText typeface unavailable")?;
    compare_typefaces(database, rasterizer, id, typeface, &system, family, weight)
}

fn find_face(database: &Database, family: &str, weight: Weight) -> Result<ID, String> {
    database
        .faces()
        .find(|face| face.weight == weight && face.families.iter().any(|(name, _)| name == family))
        .map(|face| face.id)
        .ok_or_else(|| format!("{family} weight {} missing", weight.0))
}

fn physical_source(
    database: &Database,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
    typeface: &skia_safe::Typeface,
) -> Result<Option<SourceProof>, String> {
    let (_, index) = rasterizer
        .face_file(id)?
        .ok_or("fontdb face is not file-backed")?;
    let Some((data, actual_index)) = typeface.to_existing_font_data() else {
        return Ok(None);
    };
    let same_bytes = database
        .with_face_data(id, |bytes, _| bytes == data.as_bytes())
        .ok_or("fontdb face data missing")?;
    Ok(Some(SourceProof {
        bytes_equal: same_bytes,
        index_equal: index == actual_index,
    }))
}

#[derive(Clone, Copy)]
struct SourceProof {
    bytes_equal: bool,
    index_equal: bool,
}

struct TypefaceMetrics {
    file_glyph_count: usize,
    system_glyph_count: usize,
    width_delta: f32,
    pixel_delta: (usize, u8),
}

fn compare_typefaces(
    database: &Database,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
    file: &skia_safe::Typeface,
    system: &skia_safe::Typeface,
    family: &str,
    weight: Weight,
) -> Result<(), String> {
    let file_source =
        physical_source(database, rasterizer, id, file)?.ok_or("file typeface data unavailable")?;
    let system_source = physical_source(database, rasterizer, id, system)?;
    if !file_source.bytes_equal || !file_source.index_equal {
        return Err("file typeface differs from its physical fontdb source".to_string());
    }
    let metrics = combined_typeface_metrics(database, id, file, system, family, weight)?;
    let face = database
        .face(id)
        .ok_or_else(|| "fontdb face disappeared".to_string())?;
    report_typeface(
        face,
        file,
        system,
        file_source,
        system_source,
        &metrics,
        weight,
    );
    Ok(())
}

fn combined_typeface_metrics(
    database: &Database,
    id: ID,
    file: &skia_safe::Typeface,
    system: &skia_safe::Typeface,
    family: &str,
    weight: Weight,
) -> Result<TypefaceMetrics, String> {
    let mut metrics = typeface_metrics(file, system)?;
    let upem = file
        .units_per_em()
        .ok_or("file typeface units-per-em unavailable")? as f32;
    metrics.pixel_delta = glyph_pixel_delta(database, id, file, system, family, weight, upem)?;
    Ok(metrics)
}

fn glyph_pixel_delta(
    database: &Database,
    id: ID,
    file: &skia_safe::Typeface,
    system: &skia_safe::Typeface,
    family: &str,
    weight: Weight,
    upem: f32,
) -> Result<(usize, u8), String> {
    let tree = parse_pixel_tree(database, family, weight)?;
    let Some(Node::Text(text)) = tree.node_by_id("run") else {
        return Err("pixel fixture text missing".to_string());
    };
    let prepared = prepare_pixel_span(text, id, upem)?;
    let file_pixels = draw_pixels(&prepared, file)?;
    let system_pixels = draw_pixels(&prepared, system)?;
    Ok(pixel_difference(&file_pixels, &system_pixels))
}

pub(super) fn parse_pixel_tree(
    database: &Database,
    family: &str,
    weight: Weight,
) -> Result<Tree, String> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" x="0" y="32" font-family="{family}" font-weight="{}" font-size="{SAMPLE_FONT_SIZE}">AV</text></svg>"#,
        weight.0,
    );
    Tree::from_str(
        &svg,
        &Options {
            fontdb: Arc::new(database.clone()),
            ..Options::default()
        },
    )
    .map_err(|error| error.to_string())
}

pub(super) fn prepare_pixel_span(
    text: &resvg::usvg::Text,
    id: ID,
    upem: f32,
) -> Result<PreparedSpan, String> {
    let span = text
        .layouted()
        .first()
        .ok_or("pixel fixture span missing")?;
    let glyph_ids = span
        .positioned_glyphs
        .iter()
        .map(|glyph| u16::try_from(glyph.id.0).map_err(|_| "glyph id overflow".to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PreparedSpan {
        fill: [0, 0, 0, OPAQUE_ALPHA],
        font: id,
        glyphs: span.positioned_glyphs.clone(),
        glyph_ids,
        upem,
        font_size: SAMPLE_FONT_SIZE,
        baseline_shift: 0.0,
        x_alignment: None,
        chunk_alignment: None,
        variable_instance: None,
    })
}

pub(super) fn draw_pixels(
    span: &PreparedSpan,
    typeface: &skia_safe::Typeface,
) -> Result<Vec<u8>, String> {
    let mut pixels = vec![0; PIXEL_WIDTH as usize * PIXEL_HEIGHT as usize * RGBA_CHANNELS];
    let info = ImageInfo::new(
        (PIXEL_WIDTH, PIXEL_HEIGHT),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut surface =
        surfaces::wrap_pixels(&info, pixels.as_mut_slice(), info.min_row_bytes(), None)
            .ok_or("pixel surface allocation failed")?;
    paint::draw_span(surface.canvas(), span, typeface, 1.0)?;
    drop(surface);
    if !pixels
        .chunks_exact(RGBA_CHANNELS)
        .any(|pixel| pixel[ALPHA_CHANNEL] != 0)
    {
        return Err("typeface produced an empty glyph mask".to_string());
    }
    Ok(pixels)
}

pub(super) fn pixel_difference(left: &[u8], right: &[u8]) -> (usize, u8) {
    left.iter()
        .zip(right)
        .fold((0, 0), |(count, maximum), (a, b)| {
            let delta = a.abs_diff(*b);
            (count + usize::from(delta != 0), maximum.max(delta))
        })
}

fn typeface_metrics(
    file: &skia_safe::Typeface,
    system: &skia_safe::Typeface,
) -> Result<TypefaceMetrics, String> {
    let file_font = Font::new(file.clone(), SAMPLE_FONT_SIZE);
    let system_font = Font::new(system.clone(), SAMPLE_FONT_SIZE);
    let file_glyphs = file_font.text_to_glyphs_vec(SAMPLE);
    let system_glyphs = system_font.text_to_glyphs_vec(SAMPLE);
    if file_glyphs.is_empty() || system_glyphs.is_empty() {
        return Err("typeface produced no glyphs".to_string());
    }
    if file_glyphs.contains(&0) || system_glyphs.contains(&0) {
        return Err("typeface produced a missing glyph".to_string());
    }
    if file_glyphs.len() != system_glyphs.len() {
        return Err("typefaces produced different glyph counts".to_string());
    }
    let file_width = file_font.measure_text(SAMPLE, None).0;
    let system_width = system_font.measure_text(SAMPLE, None).0;
    Ok(TypefaceMetrics {
        file_glyph_count: file_glyphs.len(),
        system_glyph_count: system_glyphs.len(),
        width_delta: file_width - system_width,
        pixel_delta: (0, 0),
    })
}

fn report_typeface(
    face: &resvg::usvg::fontdb::FaceInfo,
    file: &skia_safe::Typeface,
    system: &skia_safe::Typeface,
    file_source: SourceProof,
    system_source: Option<SourceProof>,
    metrics: &TypefaceMetrics,
    weight: Weight,
) {
    eprintln!(
        "typeface family={FAMILY} weight={} postscript={} system_postscript={} resource_equal={} file_bytes_equal={} file_index_equal={} system_source={} system_bytes_equal={:?} system_index_equal={:?} glyphs={}/{} width_delta={:.4} pixel_delta_count={} pixel_delta_max={}",
        weight.0,
        face.post_script_name,
        system.post_script_name().unwrap_or_default(),
        file.resource_name() == system.resource_name(),
        file_source.bytes_equal,
        file_source.index_equal,
        system_source.is_some(),
        system_source.map(|source| source.bytes_equal),
        system_source.map(|source| source.index_equal),
        metrics.file_glyph_count,
        metrics.system_glyph_count,
        metrics.width_delta,
        metrics.pixel_delta.0,
        metrics.pixel_delta.1,
    );
}
