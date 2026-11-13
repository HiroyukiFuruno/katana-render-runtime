use super::super::super::{
    FontShape, shape_fragment_run, svg_run_endpoints_with_face, svg_run_x_height_with_face,
};
use super::{
    FONT_BYTES, HHEA_ASCENDER_OFFSET, OS2_FS_SELECTION_OFFSET, cached_face_line_metrics,
    diagnosed_cached_face_line_metrics, font, metrics_from_font, oracle, set_word,
    static_library_metrics,
};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use crate::markdown::svg_rasterize::text_shaping::test_font_data::font_with_empty_table;
use resvg::usvg::fontdb::{Database, ID};
use rustybuzz::ttf_parser::Tag;
use skrifa::raw::{ReadError, TableProvider};
use std::path::PathBuf;

type TestResult = Result<(), String>;
const SFNT_HEADER_LENGTH: usize = 12;
const SFNT_TABLE_RECORD_LENGTH: usize = 16;
const SFNT_TAG_LENGTH: usize = 4;
const SFNT_WORD_LENGTH: usize = std::mem::size_of::<u16>();
const SFNT_NUM_TABLES_OFFSET: usize = SFNT_TAG_LENGTH;
const SFNT_SEARCH_RANGE_OFFSET: usize = SFNT_NUM_TABLES_OFFSET + SFNT_WORD_LENGTH;
const SFNT_ENTRY_SELECTOR_OFFSET: usize = SFNT_SEARCH_RANGE_OFFSET + SFNT_WORD_LENGTH;
const SFNT_RANGE_SHIFT_OFFSET: usize = SFNT_ENTRY_SELECTOR_OFFSET + SFNT_WORD_LENGTH;
const METRIC_TEST_SIZE: f32 = 15.0;
const METRIC_TEST_WEIGHT: u16 = 400;
const HEAD_UNITS_PER_EM_OFFSET: usize = 18;
const NORMALIZED_UNITS_PER_EM: u16 = 1000;
const OS2_USE_TYPO_METRICS: u16 = 1 << 7;
const OS2_TYPO_ASCENDER_OFFSET: usize = 68;
const OS2_TYPO_DESCENDER_OFFSET: usize = 70;
const OS2_X_HEIGHT_OFFSET: usize = 86;
const INVALID_X_HEIGHT: i16 = -1;
const VARIABLE_FONT: &str =
    "../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/RobotoFlex.subset.ttf";

#[test]
fn absent_face_returns_no_metrics_for_both_readers() -> TestResult {
    let database = Database::new();
    let missing = ID::default();
    SvgRasterizeOps::with_html_font_render_scope(|| {
        assert_eq!(
            cached_face_line_metrics(&database, missing, METRIC_TEST_SIZE),
            Ok(None)
        );
        assert_eq!(
            diagnosed_cached_face_line_metrics(&database, missing, METRIC_TEST_SIZE),
            Ok(None)
        );
        Ok(())
    })
}

#[test]
fn read_fonts_normalizes_zero_units_per_em_from_a_font_header() -> TestResult {
    let mut bytes = FONT_BYTES.to_vec();
    set_word(&mut bytes, b"head", HEAD_UNITS_PER_EM_OFFSET, 0)?;
    let parsed = font(&bytes)?;
    let head = parsed.tables().head().map_err(|error| error.to_string())?;
    assert_eq!(head.units_per_em(), 0);
    assert_eq!(parsed.metrics().units_per_em, NORMALIZED_UNITS_PER_EM);
    Ok(())
}

#[test]
fn negative_os2_x_height_is_rejected_for_a_parseable_font() -> TestResult {
    let mut bytes = FONT_BYTES.to_vec();
    set_word(
        &mut bytes,
        b"OS/2",
        OS2_X_HEIGHT_OFFSET,
        u16::from_be_bytes(INVALID_X_HEIGHT.to_be_bytes()),
    )?;
    let invalid = font(&bytes)?;
    let os2 = invalid.tables().os2().map_err(|error| error.to_string())?;
    assert_eq!(os2.sx_height(), Some(INVALID_X_HEIGHT));
    let shape = FontShape::new(METRIC_TEST_SIZE, METRIC_TEST_WEIGHT, false);
    for (bytes, usable) in [(FONT_BYTES.to_vec(), true), (bytes, false)] {
        let (database, id) = loaded_font(bytes)?;
        let actual = svg_run_x_height_with_face(&database, id, shape);
        if usable {
            assert!(actual.flatten().is_some_and(|height| height > 0.0));
        } else {
            assert_eq!(actual, None);
        }
    }
    Ok(())
}

#[test]
fn equal_line_endpoints_are_rejected_for_a_parseable_font() -> TestResult {
    let shape = FontShape::new(METRIC_TEST_SIZE, METRIC_TEST_WEIGHT, false);
    let (control, control_id) = loaded_font(FONT_BYTES.to_vec())?;
    assert!(svg_run_endpoints_with_face(&control, control_id, shape).is_some());
    assert!(shape_fragment_run(&control, control_id, "AV", shape)?.is_some());
    let (database, id) = loaded_font(font_with_zero_line_endpoints()?)?;
    assert_eq!(svg_run_endpoints_with_face(&database, id, shape), None);
    assert_eq!(
        shape_fragment_run(&database, id, "AV", shape)
            .err()
            .as_deref(),
        Some("invalid SVG run metrics")
    );
    Ok(())
}

fn font_with_zero_line_endpoints() -> Result<Vec<u8>, String> {
    let mut bytes = FONT_BYTES.to_vec();
    set_word(
        &mut bytes,
        b"OS/2",
        OS2_FS_SELECTION_OFFSET,
        OS2_USE_TYPO_METRICS,
    )?;
    for field in [OS2_TYPO_ASCENDER_OFFSET, OS2_TYPO_DESCENDER_OFFSET] {
        set_word(&mut bytes, b"OS/2", field, 0)?;
    }
    let invalid = font(&bytes)?;
    let line = invalid
        .metrics()
        .h_line()
        .ok_or("fixture metrics missing")?;
    assert_eq!(line.ascender.to_f32(), 0.0);
    assert_eq!(line.descender, line.ascender);
    Ok(bytes)
}

fn loaded_font(bytes: Vec<u8>) -> Result<(Database, ID), String> {
    let mut database = Database::new();
    database.load_font_data(bytes);
    let id = database.faces().next().ok_or("font fixture missing")?.id;
    Ok((database, id))
}

#[test]
fn variable_face_tables_do_not_enter_static_line_metrics() -> TestResult {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(VARIABLE_FONT);
    let mut bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    assert_variable_face_rejects_static_metrics(&bytes, b"fvar")?;
    remove_sfnt_record(&mut bytes, b"fvar")?;
    assert_variable_face_rejects_static_metrics(&bytes, b"avar")?;
    remove_sfnt_record(&mut bytes, b"avar")?;
    assert_variable_face_rejects_static_metrics(&bytes, b"MVAR")
}

fn assert_variable_face_rejects_static_metrics(
    bytes: &[u8],
    remaining: &[u8; SFNT_TAG_LENGTH],
) -> TestResult {
    let parsed = font(bytes)?;
    let tables = parsed.tables();
    assert_eq!(tables.fvar().is_ok(), remaining == b"fvar");
    assert_eq!(tables.avar().is_ok(), remaining != b"MVAR");
    assert!(tables.mvar().is_ok());
    assert!(metrics_from_font(&parsed, METRIC_TEST_SIZE).is_none());
    assert!(
        super::super::metrics_from_tables(tables, METRIC_TEST_SIZE, || Some((1.0, 1.0))).is_none(),
        "remaining table: {remaining:?}"
    );
    Ok(())
}

#[test]
fn absent_os2_falls_back_to_full_parser_metrics() -> TestResult {
    let mut bytes = FONT_BYTES.to_vec();
    remove_sfnt_record(&mut bytes, b"OS/2")?;
    set_word(&mut bytes, b"hhea", HHEA_ASCENDER_OFFSET, 0)?;
    let font = font(&bytes)?;
    assert!(font.tables().os2().is_err());
    let expected = oracle(&bytes, METRIC_TEST_SIZE);
    assert!(expected.is_some());
    assert!(static_library_metrics(&font, METRIC_TEST_SIZE).is_none());
    assert_eq!(metrics_from_font(&font, METRIC_TEST_SIZE), expected);
    Ok(())
}

#[test]
fn malformed_os2_bypasses_cached_metrics_and_preserves_legacy_metrics() -> TestResult {
    let bytes = font_with_empty_table(FONT_BYTES, Tag::from_bytes(b"OS/2"))?;
    let direct = font(&bytes)?;
    assert!(
        direct
            .tables()
            .os2()
            .is_err_and(|error| { !matches!(error, ReadError::TableIsMissing(_)) })
    );
    let expected = oracle(&bytes, METRIC_TEST_SIZE).ok_or("legacy line metrics missing")?;
    let (database, id) = loaded_font(bytes)?;
    SvgRasterizeOps::with_html_font_render_scope(|| {
        assert_eq!(
            cached_face_line_metrics(&database, id, METRIC_TEST_SIZE),
            Ok(None)
        );
        assert_eq!(
            diagnosed_cached_face_line_metrics(&database, id, METRIC_TEST_SIZE),
            Ok(None)
        );
        let legacy = database.with_face_data(id, |data, _| oracle(data, METRIC_TEST_SIZE));
        assert_eq!(legacy.flatten(), Some(expected));
        Ok(())
    })
}

fn remove_sfnt_record(bytes: &mut [u8], tag: &[u8; SFNT_TAG_LENGTH]) -> TestResult {
    let mut records = sfnt_records(bytes)?;
    let original_count = records.len();
    records.retain(|record| &record[..SFNT_TAG_LENGTH] != tag);
    if records.len() == original_count {
        return Err(format!("SFNT directory record {tag:?} missing"));
    }
    rewrite_sfnt_directory(bytes, &records)
}

fn sfnt_records(bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let count = usize::from(u16::from_be_bytes([
        bytes[SFNT_NUM_TABLES_OFFSET],
        bytes[SFNT_NUM_TABLES_OFFSET + 1],
    ]));
    let end = SFNT_HEADER_LENGTH + count * SFNT_TABLE_RECORD_LENGTH;
    Ok(bytes[SFNT_HEADER_LENGTH..end]
        .chunks_exact(SFNT_TABLE_RECORD_LENGTH)
        .map(<[u8]>::to_vec)
        .collect())
}

fn rewrite_sfnt_directory(bytes: &mut [u8], records: &[Vec<u8>]) -> TestResult {
    let count = records.len();
    let selector = usize::BITS - 1 - count.leading_zeros();
    let power = 1usize << selector;
    let search_range =
        u16::try_from(power * SFNT_TABLE_RECORD_LENGTH).map_err(|error| error.to_string())?;
    let count = u16::try_from(count).map_err(|error| error.to_string())?;
    let range_shift =
        u16::try_from(usize::from(count) * SFNT_TABLE_RECORD_LENGTH - usize::from(search_range))
            .map_err(|error| error.to_string())?;
    let directory = records.concat();
    bytes[SFNT_HEADER_LENGTH..SFNT_HEADER_LENGTH + directory.len()].copy_from_slice(&directory);
    bytes[SFNT_NUM_TABLES_OFFSET..SFNT_SEARCH_RANGE_OFFSET].copy_from_slice(&count.to_be_bytes());
    bytes[SFNT_SEARCH_RANGE_OFFSET..SFNT_ENTRY_SELECTOR_OFFSET]
        .copy_from_slice(&search_range.to_be_bytes());
    bytes[SFNT_ENTRY_SELECTOR_OFFSET..SFNT_RANGE_SHIFT_OFFSET].copy_from_slice(
        &u16::try_from(selector)
            .map_err(|error| error.to_string())?
            .to_be_bytes(),
    );
    bytes[SFNT_RANGE_SHIFT_OFFSET..SFNT_HEADER_LENGTH].copy_from_slice(&range_shift.to_be_bytes());
    Ok(())
}
