use super::{
    FONT_BYTES, HHEA_ASCENDER_OFFSET, cached_face_line_metrics, diagnosed_cached_face_line_metrics,
    font, metrics_from_font, oracle, set_word, static_library_metrics,
};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use read_fonts::TableProvider;
use resvg::usvg::fontdb::{Database, ID};
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
fn negative_os2_x_height_is_rejected_for_a_parseable_font() -> TestResult {
    use super::super::super::{FontShape, svg_run_x_height_with_face};
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
    let shape = FontShape::new(METRIC_TEST_SIZE, 400, false);
    for (bytes, usable) in [(FONT_BYTES.to_vec(), true), (bytes, false)] {
        let mut database = Database::new();
        database.load_font_data(bytes);
        let id = database.faces().next().ok_or("font fixture missing")?.id;
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
fn variable_face_tables_do_not_enter_static_line_metrics() -> TestResult {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(VARIABLE_FONT);
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    let font = font(&bytes)?;
    assert!(font.tables().fvar().is_ok());
    assert!(metrics_from_font(&font, METRIC_TEST_SIZE).is_none());
    assert!(super::super::metrics_from_tables(font.tables(), METRIC_TEST_SIZE).is_none());
    Ok(())
}

#[test]
fn absent_os2_falls_back_to_full_parser_metrics() -> TestResult {
    let mut bytes = FONT_BYTES.to_vec();
    remove_os2_record(&mut bytes)?;
    set_word(&mut bytes, b"hhea", HHEA_ASCENDER_OFFSET, 0)?;
    let font = font(&bytes)?;
    assert!(font.tables().os2().is_err());
    let expected = oracle(&bytes, METRIC_TEST_SIZE);
    assert!(expected.is_some());
    assert!(static_library_metrics(&font, METRIC_TEST_SIZE).is_none());
    assert_eq!(metrics_from_font(&font, METRIC_TEST_SIZE), expected);
    Ok(())
}

fn remove_os2_record(bytes: &mut [u8]) -> TestResult {
    let mut records = sfnt_records(bytes)?;
    let original_count = records.len();
    records.retain(|record| &record[..SFNT_TAG_LENGTH] != b"OS/2");
    if records.len() == original_count {
        return Err("OS/2 directory record missing".into());
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
