use super::super::{HTML_FONT_MANAGER, HtmlGlyphRasterizer, ttc};
use super::{FAMILY, find_face};
use resvg::usvg::fontdb::{Database, ID, Weight};
use serde_json::{Value, json};
use skia_safe::{Data, FontMgr, Typeface};
use std::collections::BTreeSet;
use std::time::Instant;

#[path = "svg_rasterize_html_glyph_file_mapping_tests.rs"]
mod mapping_tests;

const ITERATIONS: usize = 30;
const SAMPLE: &str = "AV";

struct SelectedSource {
    path: std::path::PathBuf,
    index: u32,
    bytes: Vec<u8>,
}

#[test]
fn file_backed_typeface_creation_paths_report_cost_and_identity() -> Result<(), String> {
    let database_started = Instant::now();
    let database = super::super::super::font::html_font_db_for_text(FAMILY, SAMPLE);
    let database_acquisition_ns = database_started.elapsed().as_nanos();
    let manager_started = Instant::now();
    let manager = HTML_FONT_MANAGER.with(Clone::clone);
    let manager_retrieval_ns = manager_started.elapsed().as_nanos();
    record_initialization(database_acquisition_ns, manager_retrieval_ns);
    for weight in [Weight::NORMAL, Weight::BOLD] {
        eprintln!("{}", profile_weight(&database, &manager, weight)?);
    }
    Ok(())
}

fn record_initialization(database_ns: u128, manager_ns: u128) {
    let empty_manager_started = Instant::now();
    let empty_manager = FontMgr::custom_empty();
    let empty_manager_initialization_ns = empty_manager_started.elapsed().as_nanos();
    let empty_manager_family_count = empty_manager.as_ref().map(FontMgr::count_families);
    eprintln!(
        "{}",
        json!({
            "probe": "selected-font-initialization",
            "family": FAMILY,
            "database_acquisition_ns": database_ns,
            "html_font_manager_retrieval_ns": manager_ns,
            "custom_empty_available": empty_manager.is_some(),
            "custom_empty_initialization_ns": empty_manager_initialization_ns,
            "custom_empty_family_count": empty_manager_family_count,
        })
    );
}

fn profile_weight(database: &Database, manager: &FontMgr, weight: Weight) -> Result<Value, String> {
    let id = find_face(database, FAMILY, weight)?;
    let source = selected_source(database, id)?;
    let data = Data::new_copy(&source.bytes);
    let fresh = fresh_rasterizers(database, id, source.index, &source.bytes)?;
    let rasterizer = HtmlGlyphRasterizer::new(database);
    let cached = repeated_ensure_face(&rasterizer, id, source.index, &source.bytes)?;
    let direct = repeated_data_creation(manager, &data, source.index, &source.bytes)?;
    let phases = time_existing_phases(&HtmlGlyphRasterizer::new(database), id, &source, manager)?;
    Ok(json!({
        "probe": "selected-file-typeface-cache",
        "family": FAMILY,
        "weight": weight.0,
        "face_index": source.index,
        "source_bytes": source.bytes.len(),
        "iterations": ITERATIONS,
        "fresh_rasterizer": fresh,
        "same_rasterizer": cached,
        "same_data_recreation": direct,
        "existing_phases": phases,
    }))
}

fn selected_source(database: &Database, id: ID) -> Result<SelectedSource, String> {
    let rasterizer = HtmlGlyphRasterizer::new(database);
    let (path, index) = rasterizer
        .face_file(id)?
        .ok_or("selected face is not file-backed")?;
    let bytes = database
        .with_face_data(id, |data, _| data.to_vec())
        .ok_or("selected physical face data is unavailable")?;
    Ok(SelectedSource { path, index, bytes })
}

fn fresh_rasterizers(
    database: &Database,
    id: ID,
    index: u32,
    bytes: &[u8],
) -> Result<Value, String> {
    let mut elapsed_ns = 0_u128;
    let mut ids = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        let started = Instant::now();
        let rasterizer = HtmlGlyphRasterizer::new(database);
        let face = rasterizer
            .ensure_face(id)?
            .ok_or("fresh file face unavailable")?;
        elapsed_ns += started.elapsed().as_nanos();
        ids.push(prove_face(face.typeface.as_skia()?, bytes, index)?);
    }
    Ok(path_profile(elapsed_ns, &ids, ITERATIONS))
}

fn repeated_ensure_face(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
    index: u32,
    bytes: &[u8],
) -> Result<Value, String> {
    let mut ids = Vec::with_capacity(ITERATIONS);
    let first_started = Instant::now();
    let first = rasterizer
        .ensure_face(id)?
        .ok_or("initial cached file face unavailable")?;
    let first_ns = first_started.elapsed().as_nanos();
    ids.push(prove_face(first.typeface.as_skia()?, bytes, index)?);
    let mut cached_ensure_total_ns = 0_u128;
    for _ in 1..ITERATIONS {
        let started = Instant::now();
        let face = rasterizer
            .ensure_face(id)?
            .ok_or("cached file face unavailable")?;
        cached_ensure_total_ns += started.elapsed().as_nanos();
        ids.push(prove_face(face.typeface.as_skia()?, bytes, index)?);
    }
    Ok(json!({
        "ensure_calls": ITERATIONS,
        "first_ensure_ns": first_ns,
        "cached_ensure_total_ns": cached_ensure_total_ns,
        "cached_ensure_calls": ITERATIONS - 1,
        "unique_typeface_ids": unique_count(&ids),
    }))
}

fn repeated_data_creation(
    manager: &FontMgr,
    data: &Data,
    index: u32,
    bytes: &[u8],
) -> Result<Value, String> {
    let mut clone_ns = 0_u128;
    let mut create_ns = 0_u128;
    let mut ids = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        let clone_started = Instant::now();
        let request_data = data.clone();
        clone_ns += clone_started.elapsed().as_nanos();
        let create_started = Instant::now();
        let face = ttc::create_physical_typeface(&request_data, index, manager)?
            .ok_or("same-data physical typeface creation unavailable")?;
        create_ns += create_started.elapsed().as_nanos();
        ids.push(prove_face(&face, bytes, index)?);
    }
    Ok(json!({
        "data_clone_calls": ITERATIONS,
        "data_clone_total_ns": clone_ns,
        "typeface_create_calls": ITERATIONS,
        "typeface_create_total_ns": create_ns,
        "unique_typeface_ids": unique_count(&ids),
    }))
}

fn time_existing_phases(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
    source: &SelectedSource,
    manager: &FontMgr,
) -> Result<Value, String> {
    let mut metadata_ns = 0_u128;
    let mut mapping_ns = 0_u128;
    let mut creation_ns = 0_u128;
    for _ in 0..ITERATIONS {
        metadata_ns += measure_metadata(rasterizer, id)?;
        let (data, elapsed) = measure_mapping(rasterizer, id, &source.path)?;
        mapping_ns += elapsed;
        creation_ns += measure_typeface_creation(manager, &data, source)?;
    }
    Ok(json!({
        "metadata_calls": ITERATIONS, "metadata_total_ns": metadata_ns,
        "map_face_data_calls": ITERATIONS, "map_face_data_total_ns": mapping_ns,
        "typeface_create_calls": ITERATIONS, "typeface_create_total_ns": creation_ns,
    }))
}

fn measure_metadata(rasterizer: &HtmlGlyphRasterizer<'_>, id: ID) -> Result<u128, String> {
    let started = Instant::now();
    if rasterizer.face_metadata(id)?.unsupported {
        return Err("selected face is unsupported by the file glyph path".to_string());
    }
    Ok(started.elapsed().as_nanos())
}

fn measure_mapping(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
    path: &std::path::Path,
) -> Result<(Data, u128), String> {
    let started = Instant::now();
    let data = rasterizer.map_face_data(id, path)?;
    Ok((data, started.elapsed().as_nanos()))
}

fn measure_typeface_creation(
    manager: &FontMgr,
    data: &Data,
    source: &SelectedSource,
) -> Result<u128, String> {
    let started = Instant::now();
    let face = ttc::create_physical_typeface(data, source.index, manager)?
        .ok_or("phase physical typeface creation unavailable")?;
    let elapsed = started.elapsed().as_nanos();
    prove_face(&face, &source.bytes, source.index)?;
    Ok(elapsed)
}

fn prove_face(face: &Typeface, bytes: &[u8], index: u32) -> Result<u32, String> {
    let (data, actual_index) = face
        .to_existing_font_data()
        .ok_or("typeface source proof unavailable")?;
    if actual_index != index || data.as_bytes() != bytes {
        return Err(
            "typeface bytes or face index differ from selected physical source".to_string(),
        );
    }
    Ok(face.unique_id())
}

fn path_profile(elapsed_ns: u128, ids: &[u32], calls: usize) -> Value {
    json!({ "rasterizer_constructions": calls, "ensure_calls": calls, "total_ns": elapsed_ns,
        "unique_typeface_ids": unique_count(ids) })
}

fn unique_count(ids: &[u32]) -> usize {
    ids.iter().copied().collect::<BTreeSet<_>>().len()
}
