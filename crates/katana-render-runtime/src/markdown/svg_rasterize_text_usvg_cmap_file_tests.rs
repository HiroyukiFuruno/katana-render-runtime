use super::{
    FONT_BYTES,
    policy_tests::{
        ENCODING_UNICODE_BMP, PLATFORM_MICROSOFT, UNICODE_BMP_GLYPH_Z, cmap_with_records,
        format4_mapping,
    },
};
use resvg::usvg::fontdb::{Database, ID};
use std::{path::Path, sync::Arc};

const SFNT_TABLE_COUNT_OFFSET: usize = 4;
const U16_FIELD_BYTES: usize = 2;
const SFNT_HEADER_BYTES: usize = 12;
const SFNT_TABLE_RECORD_BYTES: usize = 16;
const SFNT_TAG_BYTES: usize = 4;
const SFNT_TABLE_OFFSET_FIELD_START: usize = 8;
const SFNT_TABLE_OFFSET_FIELD_END: usize = 12;
const SFNT_TABLE_LENGTH_FIELD_START: usize = 12;
const SFNT_TABLE_LENGTH_FIELD_END: usize = 16;
const SFNT_TABLE_ALIGNMENT_BYTES: usize = 4;

pub(super) fn replace_cmap_table(font: &[u8], cmap: &[u8]) -> Result<Vec<u8>, String> {
    let table_count = u16::from_be_bytes(
        font[SFNT_TABLE_COUNT_OFFSET..SFNT_TABLE_COUNT_OFFSET + U16_FIELD_BYTES]
            .try_into()
            .map_err(|error: std::array::TryFromSliceError| error.to_string())?,
    ) as usize;
    let cmap_record = (0..table_count)
        .map(|index| SFNT_HEADER_BYTES + index * SFNT_TABLE_RECORD_BYTES)
        .find(|offset| &font[*offset..*offset + SFNT_TAG_BYTES] == b"cmap")
        .ok_or("font has no cmap record")?;
    let mut result = font.to_vec();
    while !result.len().is_multiple_of(SFNT_TABLE_ALIGNMENT_BYTES) {
        result.push(0);
    }
    let offset = u32::try_from(result.len()).map_err(|error| error.to_string())?;
    let length = u32::try_from(cmap.len()).map_err(|error| error.to_string())?;
    result[cmap_record + SFNT_TABLE_OFFSET_FIELD_START..cmap_record + SFNT_TABLE_OFFSET_FIELD_END]
        .copy_from_slice(&offset.to_be_bytes());
    result[cmap_record + SFNT_TABLE_LENGTH_FIELD_START..cmap_record + SFNT_TABLE_LENGTH_FIELD_END]
        .copy_from_slice(&length.to_be_bytes());
    result.extend_from_slice(cmap);
    Ok(result)
}

pub(super) struct TempFont(pub(super) std::path::PathBuf);

impl TempFont {
    pub(super) fn write(bytes: &[u8]) -> Result<Self, String> {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "krr-usvg-cmap-semantics-{}-{}.ttf",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
        Ok(Self(path))
    }
}

impl Drop for TempFont {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn file_database(path: &Path) -> Result<(Arc<Database>, resvg::usvg::fontdb::ID), String> {
    let mut database = Database::new();
    database
        .load_font_file(path)
        .map_err(|error| error.to_string())?;
    let database = Arc::new(database);
    let id = database
        .faces()
        .next()
        .map(|face| face.id)
        .ok_or("font face missing")?;
    Ok((database, id))
}

pub(super) fn with_cache_scope<T>(operation: impl FnMut() -> T) -> T {
    super::super::super::with_validated_tree_parse(operation)
}

fn has_char_twice(
    database: &Arc<Database>,
    face_id: ID,
    character: char,
) -> (Result<bool, ()>, Result<bool, ()>) {
    with_cache_scope(|| {
        (
            super::super::has_char(database, face_id, character),
            super::super::has_char(database, face_id, character),
        )
    })
}

#[test]
fn file_generation_change_and_missing_path_never_reuse_old_result() -> Result<(), String> {
    let no_a = replace_cmap_table(
        FONT_BYTES,
        &cmap_with_records(&[(
            PLATFORM_MICROSOFT,
            ENCODING_UNICODE_BMP,
            format4_mapping('Z' as u16, UNICODE_BMP_GLYPH_Z),
        )]),
    )?;
    let font = TempFont::write(FONT_BYTES)?;
    let (database, face_id) = file_database(&font.0)?;
    assert_eq!(
        has_char_twice(&database, face_id, 'A'),
        (Ok(true), Ok(true))
    );
    let replacement = font.0.with_extension("replacement.ttf");
    std::fs::write(&replacement, no_a).map_err(|error| error.to_string())?;
    std::fs::rename(&replacement, &font.0).map_err(|error| error.to_string())?;
    assert_eq!(
        has_char_twice(&database, face_id, 'A'),
        (Ok(false), Ok(false))
    );
    std::fs::remove_file(&font.0).map_err(|error| error.to_string())?;
    assert!(with_cache_scope(|| super::super::has_char(&database, face_id, 'A')).is_err());
    Ok(())
}
