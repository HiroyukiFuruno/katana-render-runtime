use super::{CORRUPTED_CMAP_SFNT_BYTES, FONT_BYTES, FileDatabase, file_database};
use crate::markdown::svg_rasterize::text_shaping::{
    font::font_source_generation_uncached, test_font_data::font_with_renamed_table,
};
use rustybuzz::ttf_parser::Tag;
use skrifa::FontRef;

#[test]
fn file_mapping_rejects_invalid_font_data_and_recovers() -> Result<(), String> {
    let file = file_database()?;
    assert_file_mapping(&file, true);
    let missing = font_with_renamed_table(
        FONT_BYTES,
        Tag::from_bytes(b"cmap"),
        Tag::from_bytes(b"cma-"),
    )?;
    let parsed = FontRef::new(&missing).map_err(|error| format!("{error:?}"))?;
    assert!(
        parsed
            .table_data(super::super::super::sfnt::CMAP_TAG)
            .is_none()
    );
    for (bytes, readable) in [
        (missing.as_slice(), false),
        (CORRUPTED_CMAP_SFNT_BYTES.as_slice(), false),
        (FONT_BYTES, true),
    ] {
        std::fs::write(&file._font.0, bytes).map_err(|error| error.to_string())?;
        assert_file_mapping(&file, readable);
    }
    Ok(())
}

fn assert_file_mapping(file: &FileDatabase, readable: bool) {
    let generation = font_source_generation_uncached(&file.database, file.face_id);
    let mapping = super::super::read_file_mapping(&file.database, file.face_id, &generation);
    assert_eq!(mapping.is_ok(), readable);
    let Ok(mapping) = mapping else {
        return;
    };
    assert_eq!(
        super::super::super::sfnt::has_character(&mapping.cmap_sfnt, mapping.mapping, 'A'),
        Some(true)
    );
}
