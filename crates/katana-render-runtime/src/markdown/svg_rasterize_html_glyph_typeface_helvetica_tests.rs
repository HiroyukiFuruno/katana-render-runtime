use super::super::HtmlGlyphRasterizer;
use resvg::usvg::fontdb::{Database, ID, Weight};
use sha2::{Digest, Sha256};
use skia_safe::{Data, FontMgr};

#[path = "svg_rasterize_html_glyph_font_metrics_tests.rs"]
mod font_metrics_tests;

const FAMILY: &str = "Helvetica";
const SAMPLE: &str = "AV";

#[test]
fn helvetica_ttc_repack_is_compared_for_light_regular_and_bold() -> Result<(), String> {
    let database = super::super::super::font::html_font_db_for_text(FAMILY, SAMPLE);
    let rasterizer = HtmlGlyphRasterizer::new(&database);
    let mut observed_nonzero_ttc = false;
    for weight in [Weight::LIGHT, Weight::NORMAL, Weight::BOLD] {
        observed_nonzero_ttc |= compare_helvetica_weight(&database, &rasterizer, weight)?;
    }
    if !observed_nonzero_ttc {
        return Err("Helvetica TTC nonzero face was not observed".to_string());
    }
    Ok(())
}

fn compare_helvetica_weight(
    database: &Database,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    weight: Weight,
) -> Result<bool, String> {
    let id = find_face(database, weight)?;
    let source = source_face(database, rasterizer, id)?;
    let (cached, cached_data, cached_index) = cached_source(rasterizer, id)?;
    compare_system_typeface(database, id, &cached, weight)?;
    if source.index != 0 && cached_index != 0 {
        return Err("Helvetica TTC cache was not converted to SFNT index zero".to_string());
    }
    let (repacked, repacked_typeface) = repacked_face(&source.bytes, source.index)?;
    assert_cached_binding(&source, &cached_data, cached_index, &repacked)?;
    compare_raster_pixels(database, id, &cached, &repacked_typeface, weight)?;
    report_face(
        &source,
        &cached,
        cached_data.as_bytes(),
        cached_index,
        &repacked,
        weight,
    );
    Ok(source.index != 0)
}

fn assert_cached_binding(
    source: &SourceFace,
    cached_data: &Data,
    cached_index: u32,
    repacked: &[u8],
) -> Result<(), String> {
    let expected: &[u8] = if source.index == 0 {
        if cached_index != 0 {
            return Err("Helvetica single-face cache index changed".to_string());
        }
        &source.bytes
    } else {
        if cached_index != 0 {
            return Err("Helvetica repacked cache index is not zero".to_string());
        }
        repacked
    };
    if cached_data.as_bytes() != expected {
        return Err("Helvetica cached Typeface bytes differ from expected source".to_string());
    }
    Ok(())
}

fn cached_source(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
) -> Result<(skia_safe::Typeface, Data, u32), String> {
    let cached = rasterizer
        .ensure_face(id)?
        .ok_or("Helvetica file typeface unavailable")?;
    let typeface = cached.typeface.as_skia()?;
    let Some((data, index)) = typeface.to_existing_font_data() else {
        return Err("Helvetica cached typeface source unavailable".to_string());
    };
    Ok((typeface.clone(), Data::new_copy(data.as_bytes()), index))
}

fn compare_system_typeface(
    database: &Database,
    id: ID,
    file: &skia_safe::Typeface,
    weight: Weight,
) -> Result<(), String> {
    let system = FontMgr::default()
        .legacy_make_typeface(Some(FAMILY), file.font_style())
        .ok_or("Helvetica CoreText typeface unavailable")?;
    let metrics = super::combined_typeface_metrics(database, id, file, &system, FAMILY, weight)?;
    eprintln!(
        "helvetica coretext weight={} postscript={} width_delta={:.4} pixel_delta_count={} pixel_delta_max={}",
        weight.0,
        system.post_script_name().unwrap_or_default(),
        metrics.width_delta,
        metrics.pixel_delta.0,
        metrics.pixel_delta.1,
    );
    Ok(())
}

struct SourceFace {
    path: std::path::PathBuf,
    bytes: Vec<u8>,
    index: u32,
}

fn source_face(
    database: &Database,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
) -> Result<SourceFace, String> {
    let Some((path, index)) = rasterizer.face_file(id)? else {
        return Err("Helvetica face is not file-backed".to_string());
    };
    let (bytes, actual_index) = database
        .with_face_data(id, |bytes, index| (bytes.to_vec(), index))
        .ok_or("Helvetica source bytes unavailable")?;
    if actual_index != index {
        return Err("Helvetica source face index changed".to_string());
    }
    Ok(SourceFace { path, bytes, index })
}

fn repacked_face(bytes: &[u8], index: u32) -> Result<(Vec<u8>, skia_safe::Typeface), String> {
    let bytes = super::super::ttc::repack_face(bytes, index)?;
    let typeface = FontMgr::default()
        .new_from_data(Data::new_copy(&bytes), Some(0))
        .ok_or("Helvetica repacked SFNT typeface unavailable")?;
    Ok((bytes, typeface))
}

fn report_face(
    source: &SourceFace,
    cached: &skia_safe::Typeface,
    cached_data: &[u8],
    cached_index: u32,
    repacked: &[u8],
    weight: Weight,
) {
    eprintln!(
        "helvetica family={FAMILY} weight={} path={} source_index={} source_sha256={} cached_index={} cached_sha256={} repacked_index=0 repacked_sha256={} postscript={} pixels_compared=true",
        weight.0,
        source.path.display(),
        source.index,
        sha256(&source.bytes),
        cached_index,
        sha256(cached_data),
        sha256(repacked),
        cached.post_script_name().unwrap_or_default(),
    );
}

fn find_face(database: &Database, weight: Weight) -> Result<ID, String> {
    database
        .faces()
        .find(|face| face.weight == weight && face.families.iter().any(|(name, _)| name == FAMILY))
        .map(|face| face.id)
        .ok_or_else(|| format!("{FAMILY} weight {} missing", weight.0))
}

fn compare_raster_pixels(
    database: &Database,
    id: ID,
    cached: &skia_safe::Typeface,
    repacked: &skia_safe::Typeface,
    weight: Weight,
) -> Result<(), String> {
    let tree = super::parse_pixel_tree(database, FAMILY, weight)?;
    let Some(resvg::usvg::Node::Text(text)) = tree.node_by_id("run") else {
        return Err("Helvetica pixel fixture text missing".to_string());
    };
    let upem = cached
        .units_per_em()
        .ok_or("Helvetica units-per-em unavailable")? as f32;
    let span = super::prepare_pixel_span(text, id, upem)?;
    let cached_pixels = super::draw_pixels(&span, cached)?;
    let repacked_pixels = super::draw_pixels(&span, repacked)?;
    let delta = super::pixel_difference(&cached_pixels, &repacked_pixels);
    eprintln!(
        "helvetica weight={} cached_vs_repacked_pixel_delta_count={} cached_vs_repacked_pixel_delta_max={}",
        weight.0, delta.0, delta.1
    );
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
