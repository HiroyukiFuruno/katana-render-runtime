use super::super::font_file_fixture::FONT_BYTES;
use super::{FontBuilder, ReadFontRef, TableProvider, Tag};

const SINGLE_TABLE_DIRECTORY_BYTES: usize = 28;

pub(super) fn change_name_table(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let font = ReadFontRef::from_index(bytes, 0).map_err(|error| format!("{error:?}"))?;
    let record = font
        .table_directory()
        .table_records()
        .iter()
        .find(|record| record.tag() == Tag::new(b"name"))
        .ok_or("name table missing")?;
    let offset = record.offset() as usize;
    let mut changed = bytes.to_vec();
    let byte = changed.get_mut(offset).ok_or("name table offset invalid")?;
    *byte ^= 1;
    Ok(changed)
}

pub(super) fn font_with_only(tag: Tag) -> Result<Vec<u8>, String> {
    let font = ReadFontRef::from_index(FONT_BYTES, 0).map_err(|error| format!("{error:?}"))?;
    let data = font.data_for_tag(tag).ok_or("source table missing")?;
    let mut builder = FontBuilder::new();
    builder.add_raw(tag, data.as_bytes());
    Ok(builder.build())
}

pub(super) fn truncated_name_font() -> Result<Vec<u8>, String> {
    let mut font = font_with_only(Tag::new(b"name"))?;
    font.truncate(SINGLE_TABLE_DIRECTORY_BYTES);
    Ok(font)
}
