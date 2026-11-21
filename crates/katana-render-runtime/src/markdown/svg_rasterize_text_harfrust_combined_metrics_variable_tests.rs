use super::{CombinedMeasurement, FONT_BYTES, FontShape, RawRunCell, combined_measurement};
use resvg::usvg::fontdb::{Database, Source};
use rustybuzz::ttf_parser::{Face, Tag};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;
const SFNT_SHA: &str = "f0b8414e5c903579bfd9dbca230a49f9063d92c6d622c6700f121a50a61e06ba";
const VARIABLE_FONT_SIZE: f32 = 18.25;
const REGULAR_WEIGHT: u16 = 400;
const BOLD_WEIGHT: u16 = 700;
const SAMPLE_LETTER_SPACING: f32 = 0.25;
const METRIC_ROUNDING_EPSILON: f32 = 0.0001;

fn recursive_input() -> TestResult<(PathBuf, Vec<u8>)> {
    let path = crate::markdown::svg_rasterize::probe_test_support::recursive_sfnt_path()?;
    let bytes = std::fs::read(&path)?;
    assert_eq!(
        Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        SFNT_SHA
    );
    Ok((path, bytes))
}

fn database(bytes: &[u8], source: Source) -> TestResult<(Arc<Database>, String)> {
    let mut parsed = Database::new();
    parsed.load_font_data(bytes.to_vec());
    assert_eq!(parsed.faces().count(), 1);
    let mut info = parsed
        .faces()
        .next()
        .ok_or("controlled face missing")?
        .clone();
    let family = info
        .families
        .first()
        .ok_or("controlled family missing")?
        .0
        .clone();
    info.source = source;
    let mut db = Database::new();
    db.push_face_info(info);
    Ok((Arc::new(db), family))
}

fn oracle(bytes: &[u8], shape: FontShape, cell: &RawRunCell) -> TestResult<Vec<i16>> {
    let mut face = Face::parse(bytes, 0)?;
    assert!(face.is_variable());
    assert!(
        face.variation_axes()
            .into_iter()
            .any(|axis| axis.tag == Tag::from_bytes(b"wght"))
    );
    for (tag, value) in super::font_variations(shape) {
        face.set_variation(Tag::from_bytes(tag.as_bytes().try_into()?), value)
            .ok_or("independent axis setup failed")?;
    }
    let coords: Vec<_> = face
        .variation_coordinates()
        .iter()
        .map(|v| v.get())
        .collect();
    assert_eq!(cell.normalized_coords.len(), coords.len());
    /* WHY: 独立パーサ間のF2.14正規化の丸めだけを1単位許す。 */
    for (actual, expected) in cell.normalized_coords.iter().zip(&coords) {
        assert!((i32::from(*actual) - i32::from(*expected)).abs() <= 1);
    }
    let scale = shape.size / f32::from(face.units_per_em());
    /* WHY: ttf-parserのMVAR整数丸めを1 design unitで上限化する。 */
    let tolerance = scale + METRIC_ROUNDING_EPSILON;
    assert!((cell.above_baseline - f32::from(face.ascender()) * scale).abs() <= tolerance);
    assert!((cell.below_baseline + f32::from(face.descender()) * scale).abs() <= tolerance);
    assert!((cell.line_gap - f32::from(face.line_gap()) * scale).abs() <= tolerance);
    Ok(coords)
}

fn mixed_database(bytes: &[u8]) -> TestResult<(Arc<Database>, String)> {
    let recursive = Face::parse(bytes, 0)?;
    let noto = Face::parse(FONT_BYTES, 0)?;
    assert!(recursive.glyph_index('A').is_some());
    assert!(recursive.glyph_index('Ω').is_none());
    assert!(noto.glyph_index('Ω').is_some());
    let (mut db, family) = database(bytes, Source::Binary(Arc::new(bytes.to_vec())))?;
    Arc::make_mut(&mut db).load_font_data(FONT_BYTES.to_vec());
    Ok((db, format!("{family}, Noto Sans")))
}

fn verify_mixed_cells(bytes: &[u8], db: &Arc<Database>, family: &str) -> TestResult<()> {
    let noto = Face::parse(FONT_BYTES, 0)?;
    let shape = FontShape::new(VARIABLE_FONT_SIZE, BOLD_WEIGHT, false);
    let base = super::super::super::matching_font_face(db, family, BOLD_WEIGHT, false)
        .ok_or("mixed base missing")?;
    let runs = super::super::super::resolved_font_runs(db, base, family, "AΩA", BOLD_WEIGHT, false);
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].0, runs[2].0);
    assert_ne!(runs[0].0, runs[1].0);
    let actual = combined_measurement(db, family, "AΩA", shape, SAMPLE_LETTER_SPACING, None)
        .ok_or("mixed combined failed")?;
    let width = super::super::super::shaped_runs_advance(db, base, family, "AΩA", shape, None)
        .ok_or("existing mixed width failed")?;
    assert_eq!(actual.advance.to_bits(), (width + 0.5).to_bits());
    assert_eq!(actual.run_cells.len(), runs.len());
    for (cell, (id, _)) in actual.run_cells.iter().zip(&runs) {
        assert_eq!(cell.face_id, *id);
        verify_face_cell(bytes, shape, base, *id, &noto, cell)?;
    }
    Ok(())
}

fn verify_face_cell(
    bytes: &[u8],
    shape: FontShape,
    base: resvg::usvg::fontdb::ID,
    id: resvg::usvg::fontdb::ID,
    noto: &Face<'_>,
    cell: &RawRunCell,
) -> TestResult<()> {
    if id == base {
        oracle(bytes, shape, cell)?;
    } else {
        let scale = shape.size / f32::from(noto.units_per_em());
        assert!(
            (cell.above_baseline - f32::from(noto.ascender()) * scale).abs()
                < METRIC_ROUNDING_EPSILON
        );
        assert!(
            (cell.below_baseline + f32::from(noto.descender()) * scale).abs()
                < METRIC_ROUNDING_EPSILON
        );
        assert!(
            (cell.line_gap - f32::from(noto.line_gap()) * scale).abs() < METRIC_ROUNDING_EPSILON
        );
    }
    Ok(())
}

fn verify_variable_source(bytes: &[u8], source: Source) -> TestResult<()> {
    let (db, family) = database(bytes, source)?;
    let mut locations = Vec::new();
    for weight in [REGULAR_WEIGHT, BOLD_WEIGHT] {
        locations.push(verify_variable_weight(bytes, &db, &family, weight)?);
    }
    assert_ne!(
        locations[0], locations[1],
        "requested weights did not reach actual instance"
    );
    Ok(())
}

fn verify_variable_weight(
    bytes: &[u8],
    database: &Arc<Database>,
    family: &str,
    weight: u16,
) -> TestResult<Vec<i16>> {
    let shape = FontShape::new(VARIABLE_FONT_SIZE, weight, false);
    let actual = combined_measurement(
        database,
        family,
        "Animal AV ffi",
        shape,
        SAMPLE_LETTER_SPACING,
        None,
    )
    .ok_or("variable combined measurement failed")?;
    let base = super::super::super::matching_font_face(database, family, weight, false)
        .ok_or("variable base missing")?;
    let width = super::super::super::shaped_runs_advance(
        database,
        base,
        family,
        "Animal AV ffi",
        shape,
        None,
    )
    .ok_or("existing variable width failed")?;
    assert_variable_measurement(&actual, width, base);
    oracle(bytes, shape, &actual.run_cells[0])
}

fn assert_variable_measurement(
    actual: &CombinedMeasurement,
    width: f32,
    base: resvg::usvg::fontdb::ID,
) {
    assert_eq!(
        actual.advance.to_bits(),
        (width + 12.0 * SAMPLE_LETTER_SPACING).to_bits()
    );
    assert_eq!(actual.run_cells.len(), 1);
    assert_eq!(actual.run_cells[0].face_id, base);
}

#[test]
fn variable_sources_read_back_weight_and_preserve_width_bits() -> TestResult<()> {
    let (path, bytes) = recursive_input()?;
    for source in [
        Source::Binary(Arc::new(bytes.clone())),
        Source::SharedFile(path.clone(), Arc::new(bytes.clone())),
        Source::File(path.clone()),
    ] {
        verify_variable_source(&bytes, source)?;
    }
    assert_eq!(
        Sha256::digest(std::fs::read(path)?)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        SFNT_SHA
    );
    Ok(())
}

#[test]
fn existing_resolver_mixed_fallback_cells_follow_each_actual_face() -> TestResult<()> {
    let (path, bytes) = recursive_input()?;
    let (db, family) = mixed_database(&bytes)?;
    verify_mixed_cells(&bytes, &db, &family)?;
    assert_eq!(
        Sha256::digest(std::fs::read(path)?)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        SFNT_SHA
    );
    Ok(())
}
