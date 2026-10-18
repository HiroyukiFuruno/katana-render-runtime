use super::{FontShape, HARFRUST_SCALE, face_cache, font_variations, shape_buffer};
use harfrust::font::{Font, metrics::ScaleF32};
use resvg::usvg::fontdb::{Database, ID, Source};
use std::sync::Arc;
#[path = "svg_rasterize_text_harfrust_combined_metrics_oracle_tests.rs"]
mod oracle_tests;
use oracle_tests::verify_case;

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const FAMILY: &str = "Noto Sans";
const SAMPLE_LETTER_SPACING: f32 = 0.25;

#[derive(Debug)]
pub(super) struct RawRunCell {
    face_id: ID,
    above_baseline: f32,
    below_baseline: f32,
    line_gap: f32,
    normalized_coords: Vec<i16>,
}

#[derive(Debug)]
struct CombinedMeasurement {
    advance: f32,
    run_cells: Vec<RawRunCell>,
}

fn combined_measurement(
    database: &Arc<Database>,
    family: &str,
    text: &str,
    shape: FontShape,
    letter_spacing: f32,
    features: Option<&str>,
) -> Option<CombinedMeasurement> {
    let base = super::super::matching_font_face(database, family, shape.weight, shape.italic)?;
    let runs =
        super::super::resolved_font_runs(database, base, family, text, shape.weight, shape.italic);
    let mut result = CombinedMeasurement {
        advance: 0.0,
        run_cells: Vec::new(),
    };
    for (face_id, run) in runs {
        let (advance, cell) = combined_run(database, face_id, &run, shape, features)?;
        result.advance += advance;
        result.run_cells.push(cell);
    }
    result.advance += text.chars().count().saturating_sub(1) as f32 * letter_spacing;
    Some(result)
}

fn combined_run(
    database: &Database,
    face_id: ID,
    text: &str,
    shape: FontShape,
    features: Option<&str>,
) -> Option<(f32, RawRunCell)> {
    combined_run_with_hook(database, face_id, text, shape, features, || {})
}

pub(super) fn combined_run_with_hook(
    database: &Database,
    face_id: ID,
    text: &str,
    shape: FontShape,
    features: Option<&str>,
    before_commit: impl FnOnce(),
) -> Option<(f32, RawRunCell)> {
    let face = face_cache::font_for_face(database, face_id)?;
    let instance = face
        .instance_builder()
        .variations(font_variations(shape))
        .build();
    let glyphs = shape_buffer(&instance, text, shape, features)?;
    let cell = raw_cell(&instance, face_id, shape.size)?;
    let advance = glyphs
        .glyph_positions()
        .iter()
        .map(|position| position.x_advance as f32 / HARFRUST_SCALE)
        .sum();
    before_commit();
    face_cache::commit_file_font(database, face_id, &face).then_some((advance, cell))
}

fn raw_cell(instance: &Font, face_id: ID, size: f32) -> Option<RawRunCell> {
    let metrics = instance.metrics();
    if metrics.units_per_em == 0 {
        return None;
    }
    let line = metrics
        .scaled(ScaleF32::from_ppem(size, metrics.units_per_em))
        .h_line()?;
    let cell = RawRunCell {
        face_id,
        above_baseline: line.ascender,
        below_baseline: -line.descender,
        line_gap: line.line_gap,
        normalized_coords: instance
            .normalized_coords()
            .iter()
            .map(|v| v.to_bits())
            .collect(),
    };
    [cell.above_baseline, cell.below_baseline, cell.line_gap]
        .into_iter()
        .all(f32::is_finite)
        .then_some(cell)
}

fn source_database(source: Source) -> Result<Arc<Database>, String> {
    let mut parsed = Database::new();
    parsed.load_font_data(FONT_BYTES.to_vec());
    let mut info = parsed.faces().next().ok_or("bundled face missing")?.clone();
    info.source = source;
    let mut database = Database::new();
    database.push_face_info(info);
    Ok(Arc::new(database))
}

#[test]
fn same_handle_cells_preserve_existing_width_for_real_sources() -> Result<(), String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts/NotoSans-Regular.ttf");
    for source in [
        Source::Binary(Arc::new(FONT_BYTES.to_vec())),
        Source::SharedFile(path.clone(), Arc::new(FONT_BYTES.to_vec())),
        Source::File(path),
    ] {
        let database = source_database(source)?;
        for (text, size, weight, features) in [
            ("Animal", 14.0, 700, None),
            ("+int age", 14.0, 400, None),
            ("AV", 18.0, 400, Some("'kern' 0")),
            ("ffi", 18.0, 400, Some("'liga' 1")),
        ] {
            verify_case(&database, text, size, weight, features)?;
        }
    }
    Ok(())
}

#[test]
fn empty_runs_have_no_cell_and_removed_face_fails_closed() -> Result<(), String> {
    let mut database = source_database(Source::Binary(Arc::new(FONT_BYTES.to_vec())))?;
    let shape = FontShape::new(18.0, 400, false);
    let empty = combined_measurement(&database, FAMILY, "", shape, 0.25, None)
        .ok_or("empty measurement failed")?;
    assert_eq!(empty.advance, 0.0);
    assert!(empty.run_cells.is_empty());
    let id = database.faces().next().ok_or("bundled face missing")?.id;
    Arc::make_mut(&mut database).remove_face(id);
    assert!(combined_run(&database, id, "AV", shape, None).is_none());
    assert!(combined_measurement(&database, FAMILY, "AV", shape, 0.0, None).is_none());
    Ok(())
}

#[path = "svg_rasterize_text_harfrust_combined_metrics_variable_tests.rs"]
mod variable_tests;
