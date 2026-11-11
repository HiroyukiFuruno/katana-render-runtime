use super::{Database, cached_face_line_metrics};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use resvg::usvg::fontdb::Source;
use std::sync::Arc;

const LINE_METRIC_TEST_SIZES: [f32; 4] = [12.0, 12.5, 16.0, 24.0];
const SFNT_HEADER_LENGTH: usize = 12;
const SFNT_TABLE_RECORD_LENGTH: usize = 16;
const SFNT_TAG_LENGTH: usize = 4;
const OS2_TYPO_GAP_OFFSET: usize = 72;
const OS2_FS_SELECTION_OFFSET: usize = 62;
const U16_BYTE_WIDTH: usize = 2;
const I16_BYTE_WIDTH: usize = 2;
const TEST_TYPO_LINE_GAP: i16 = 200;
const USE_TYPO_METRICS_FLAG: u16 = 1 << 7;

#[test]
fn library_table_line_metrics_match_full_parser_for_real_static_face() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").to_vec());
    let id = database.faces().next().ok_or("real font face absent")?.id;
    SvgRasterizeOps::with_html_font_render_scope(|| {
        for size in LINE_METRIC_TEST_SIZES {
            let expected = database
                .with_face_data(id, |data, index| legacy_metrics(data, index, size))
                .flatten()
                .ok_or("legacy real font metrics absent")?;
            let actual = cached_face_line_metrics(&database, id, size)
                .map_err(|()| "generation or poison rejected")?
                .ok_or("cached real font metrics absent")?;
            assert_eq!(actual, expected);
        }
        Ok(())
    })
}

pub(super) fn legacy_metrics(data: &[u8], index: u32, size: f32) -> Option<(f32, f32)> {
    let face = rustybuzz::ttf_parser::Face::parse(data, index).ok()?;
    let scale = size / face.units_per_em() as f32;
    let leading = (face.line_gap() as f32 * scale).round();
    let before = (leading / 2.0).floor();
    Some((
        (face.ascender() as f32 * scale).round() + before,
        (-f32::from(face.descender()) * scale).round() + leading - before,
    ))
}

#[test]
fn unsorted_real_face_uses_legacy_metrics() -> Result<(), String> {
    directory_fallback_metrics(false)
}

#[test]
fn duplicate_real_face_uses_legacy_metrics() -> Result<(), String> {
    directory_fallback_metrics(true)
}

fn directory_fallback_metrics(duplicate: bool) -> Result<(), String> {
    let database = modified_real_database(duplicate)?;
    let reference = metric_reference_font(duplicate)?;
    let id = database
        .faces()
        .next()
        .ok_or("modified real face absent")?
        .id;
    SvgRasterizeOps::with_html_font_render_scope(|| {
        for size in LINE_METRIC_TEST_SIZES {
            check_directory_fallback(&database, id, size, &reference)?;
            if !duplicate {
                check_unsorted_os2_metrics(&database, id, size, &reference)?;
            }
        }
        Ok(())
    })
}

fn check_directory_fallback(
    database: &Database,
    id: resvg::usvg::fontdb::ID,
    size: f32,
    reference: &[u8],
) -> Result<(), String> {
    let expected = legacy_metrics(reference, 0, size).ok_or("original real metrics absent")?;
    let legacy = database
        .with_face_data(id, |data, index| legacy_metrics(data, index, size))
        .flatten()
        .ok_or("modified real legacy metrics absent")?;
    assert_eq!(legacy, expected);
    let bridge = cached_face_line_metrics(database, id, size)
        .map_err(|()| "modified face generation rejected")?;
    assert_eq!(bridge, None);
    Ok(())
}

fn modified_real_font(duplicate: bool) -> Result<Vec<u8>, String> {
    let mut data = metric_reference_font(duplicate)?;
    let face = read_fonts::FontRef::new(&data).map_err(|error| error.to_string())?;
    let count = face.table_directory().table_records().len();
    let directory_end = SFNT_HEADER_LENGTH + count * SFNT_TABLE_RECORD_LENGTH;
    let mut entries = data
        .get(SFNT_HEADER_LENGTH..directory_end)
        .ok_or("real SFNT directory absent")?
        .chunks_exact(SFNT_TABLE_RECORD_LENGTH)
        .map(<[u8]>::to_vec)
        .collect::<Vec<_>>();
    if duplicate {
        duplicate_os2_record(&mut entries)?;
    } else {
        let os2 = entries
            .iter()
            .position(|entry| &entry[..SFNT_TAG_LENGTH] == b"OS/2")
            .ok_or("real OS2 absent")?;
        let record = entries.remove(os2);
        entries.push(record);
    }
    data.get_mut(SFNT_HEADER_LENGTH..directory_end)
        .ok_or("real SFNT directory absent")?
        .copy_from_slice(&entries.concat());
    Ok(data)
}

fn duplicate_os2_record(entries: &mut [Vec<u8>]) -> Result<(), String> {
    let os2 = entries
        .iter()
        .find(|entry| &entry[..SFNT_TAG_LENGTH] == b"OS/2")
        .ok_or("real OS2 absent")?
        .clone();
    let post = entries
        .iter_mut()
        .find(|entry| &entry[..SFNT_TAG_LENGTH] == b"post")
        .ok_or("real post absent")?;
    *post = os2;
    entries.sort_by(|left, right| left[..SFNT_TAG_LENGTH].cmp(&right[..SFNT_TAG_LENGTH]));
    Ok(())
}

fn modified_real_database(duplicate: bool) -> Result<Database, String> {
    let mut original = Database::new();
    original.load_font_data(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").to_vec());
    let mut info = original
        .faces()
        .next()
        .ok_or("original real face absent")?
        .clone();
    info.source = Source::Binary(Arc::new(modified_real_font(duplicate)?));
    let mut database = Database::new();
    database.push_face_info(info);
    Ok(database)
}

fn metric_reference_font(duplicate: bool) -> Result<Vec<u8>, String> {
    let mut data = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").to_vec();
    if duplicate {
        return Ok(data);
    }
    let face = read_fonts::FontRef::new(&data).map_err(|error| error.to_string())?;
    let offset = face
        .table_directory()
        .table_records()
        .iter()
        .find(|record| record.tag() == read_fonts::types::Tag::new(b"OS/2"))
        .ok_or("real OS2 absent")?
        .offset() as usize;
    /* WHY: typo値を明示選択し、実NotoのOS/2 line gapが計測に反映されるfixtureにする。 */
    let typo_gap_start = offset + OS2_TYPO_GAP_OFFSET;
    let typo_gap_end = typo_gap_start + I16_BYTE_WIDTH;
    data.get_mut(typo_gap_start..typo_gap_end)
        .ok_or("real OS2 typo gap absent")?
        .copy_from_slice(&TEST_TYPO_LINE_GAP.to_be_bytes());
    let selection_start = offset + OS2_FS_SELECTION_OFFSET;
    let selection_end = selection_start + U16_BYTE_WIDTH;
    let selection = data
        .get(selection_start..selection_end)
        .ok_or("real OS2 selection absent")?;
    let flags = u16::from_be_bytes([selection[0], selection[1]]) | USE_TYPO_METRICS_FLAG;
    data.get_mut(selection_start..selection_end)
        .ok_or("real OS2 selection absent")?
        .copy_from_slice(&flags.to_be_bytes());
    Ok(data)
}

fn check_unsorted_os2_metrics(
    database: &Database,
    id: resvg::usvg::fontdb::ID,
    size: f32,
    reference: &[u8],
) -> Result<(), String> {
    let expected = legacy_metrics(reference, 0, size).ok_or("reference metrics absent")?;
    let original = legacy_metrics(
        include_bytes!("../../assets/fonts/NotoSans-Regular.ttf"),
        0,
        size,
    )
    .ok_or("original real metrics absent")?;
    assert_ne!(
        expected, original,
        "OS/2 fixture must affect legacy metrics"
    );
    let font = database
        .with_face_data(id, |data, index| {
            harfrust::font::Font::new(data.to_vec(), index)
        })
        .flatten()
        .ok_or("unguarded real font absent")?;
    let unguarded = super::metrics_from_tables(font.tables(), size, || None)
        .ok_or("unguarded mandatory metrics absent")?;
    assert_eq!(unguarded, expected);
    Ok(())
}
