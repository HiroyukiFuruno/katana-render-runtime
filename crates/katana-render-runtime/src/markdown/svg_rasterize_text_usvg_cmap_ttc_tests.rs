use super::{
    FONT_BYTES,
    file_tests::{self, TempFont, replace_cmap_table},
    policy_tests::{
        ENCODING_UNICODE_BMP, PLATFORM_MICROSOFT, UNICODE_BMP_GLYPH_Z, cmap_with_records,
        format4_mapping,
    },
};
use resvg::usvg::fontdb::Database;
use skrifa::FontRef;
use std::sync::Arc;

const SFNT_TABLE_COUNT_OFFSET: usize = 4;
const U16_FIELD_BYTES: usize = 2;
const SFNT_HEADER_BYTES: usize = 12;
const SFNT_TABLE_RECORD_BYTES: usize = 16;
const SFNT_TABLE_OFFSET_FIELD_OFFSET: usize = 8;
const U32_FIELD_BYTES: usize = 4;
const TTC_HEADER_PREFIX_BYTES: usize = 12;
const TTC_FACE_COUNT: usize = 2;
const TTC_HEADER_BYTES: usize = TTC_HEADER_PREFIX_BYTES + TTC_FACE_COUNT * U32_FIELD_BYTES;
const TTC_FIRST_FACE_OFFSET: usize = TTC_HEADER_BYTES;
const TTC_V1_SIGNATURE: &[u8; 4] = b"ttcf";
const TTC_VERSION_1_0: [u8; 4] = [0, 1, 0, 0];
const FIRST_TTC_FACE_INDEX: usize = 0;
const SECOND_TTC_FACE_INDEX: usize = 1;

fn append_ttc_face(
    ttc: &mut Vec<u8>,
    font: &[u8],
    table_count: usize,
    base: usize,
) -> Result<(), String> {
    let start = ttc.len();
    ttc.extend_from_slice(font);
    for index in 0..table_count {
        let field = start
            + SFNT_HEADER_BYTES
            + index * SFNT_TABLE_RECORD_BYTES
            + SFNT_TABLE_OFFSET_FIELD_OFFSET;
        let old = u32::from_be_bytes(
            ttc.get(field..field + U32_FIELD_BYTES)
                .ok_or("malformed SFNT table directory")?
                .try_into()
                .map_err(|error: std::array::TryFromSliceError| error.to_string())?,
        );
        ttc[field..field + U32_FIELD_BYTES].copy_from_slice(&(old + base as u32).to_be_bytes());
    }
    Ok(())
}

fn two_face_ttc(first: &[u8], second: &[u8]) -> Result<Vec<u8>, String> {
    let table_count = u16::from_be_bytes(
        first[SFNT_TABLE_COUNT_OFFSET..SFNT_TABLE_COUNT_OFFSET + U16_FIELD_BYTES]
            .try_into()
            .map_err(|error: std::array::TryFromSliceError| error.to_string())?,
    ) as usize;
    let second_table_count = u16::from_be_bytes(
        second[SFNT_TABLE_COUNT_OFFSET..SFNT_TABLE_COUNT_OFFSET + U16_FIELD_BYTES]
            .try_into()
            .map_err(|error: std::array::TryFromSliceError| error.to_string())?,
    ) as usize;
    if second_table_count != table_count {
        return Err("TTC fixture faces must have matching table counts".into());
    }
    let second_offset = TTC_HEADER_BYTES + first.len();
    let mut ttc = Vec::with_capacity(second_offset + second.len());
    ttc.extend_from_slice(TTC_V1_SIGNATURE);
    ttc.extend_from_slice(&TTC_VERSION_1_0);
    ttc.extend_from_slice(&(TTC_FACE_COUNT as u32).to_be_bytes());
    ttc.extend_from_slice(&(TTC_FIRST_FACE_OFFSET as u32).to_be_bytes());
    ttc.extend_from_slice(&(second_offset as u32).to_be_bytes());
    append_ttc_face(&mut ttc, first, table_count, TTC_FIRST_FACE_OFFSET)?;
    append_ttc_face(&mut ttc, second, table_count, second_offset)?;
    Ok(ttc)
}

fn with_cache_scope<T>(operation: impl FnMut() -> T) -> T {
    file_tests::with_cache_scope(operation)
}

fn assert_selector_matches_stock(
    first: &[u8],
    second: &[u8],
    base_face_index: usize,
    expect_match: bool,
) -> Result<(), String> {
    let bytes = two_face_ttc(first, second)?;
    let font = TempFont::write(&bytes)?;
    let database = load_database(&font)?;
    let faces: Vec<_> = database.faces().collect();
    let base_face = faces.get(base_face_index).ok_or("base TTC face missing")?;
    let excluded = [base_face.id];
    let stock = resvg::usvg::FontResolver::default_fallback_selector();
    let cached = super::super::html_selector();
    let (stock_result, cached_result) = with_cache_scope(|| {
        (
            stock('A', &excluded, &mut Arc::clone(&database)),
            cached('A', &excluded, &mut Arc::clone(&database)),
        )
    });
    assert_eq!(stock_result.is_some(), expect_match);
    assert_eq!(
        stock_result.and_then(|id| database.face(id).map(|face| face.index)),
        cached_result.and_then(|id| database.face(id).map(|face| face.index)),
    );
    Ok(())
}

fn load_database(font: &TempFont) -> Result<Arc<Database>, String> {
    let mut database = Database::new();
    database
        .load_font_file(&font.0)
        .map_err(|error| error.to_string())?;
    Ok(Arc::new(database))
}

fn assert_ttc_faces(
    database: &Arc<Database>,
    faces: &[&resvg::usvg::fontdb::FaceInfo],
) -> Result<(), String> {
    let expected_support = [(FIRST_TTC_FACE_INDEX, true), (SECOND_TTC_FACE_INDEX, false)];
    for (face_index, expected) in expected_support {
        let face = &faces[face_index];
        assert_eq!(
            database.face_source(face.id).map(|(_, index)| index),
            Some(face.index)
        );
        let stock = database
            .with_face_data(face.id, |data, index| {
                let font = FontRef::from_index(data, index).ok()?;
                Some(skrifa::charmap::Charmap::new(&font).map('A').is_some())
            })
            .flatten()
            .ok_or("stock TTC face probe failed")?;
        assert_eq!(stock, expected);
        let probes = with_cache_scope(|| {
            (
                super::super::has_char(database, face.id, 'A'),
                super::super::has_char(database, face.id, 'A'),
            )
        });
        assert_eq!(probes, (Ok(expected), Ok(expected)));
    }
    Ok(())
}

fn font_without_a() -> Result<Vec<u8>, String> {
    replace_cmap_table(
        FONT_BYTES,
        &cmap_with_records(&[(
            PLATFORM_MICROSOFT,
            ENCODING_UNICODE_BMP,
            format4_mapping('Z' as u16, UNICODE_BMP_GLYPH_Z),
        )]),
    )
}

#[test]
fn ttc_face_indices_keep_separate_stock_and_cached_cmap_results() -> Result<(), String> {
    let no_a = font_without_a()?;
    let bytes = two_face_ttc(FONT_BYTES, &no_a)?;
    let font = TempFont::write(&bytes)?;
    let mut font_database = Database::new();
    font_database
        .load_font_file(&font.0)
        .map_err(|error| error.to_string())?;
    let database = Arc::new(font_database);
    let faces: Vec<_> = database.faces().collect();
    if faces.len() != TTC_FACE_COUNT {
        return Err(format!(
            "expected {} TTC faces, got {}",
            TTC_FACE_COUNT,
            faces.len()
        ));
    }
    assert_ttc_faces(&database, &faces)?;
    assert_selector_matches_stock(FONT_BYTES, no_a.as_slice(), SECOND_TTC_FACE_INDEX, true)?;
    assert_selector_matches_stock(
        no_a.as_slice(),
        no_a.as_slice(),
        FIRST_TTC_FACE_INDEX,
        false,
    )?;
    Ok(())
}
