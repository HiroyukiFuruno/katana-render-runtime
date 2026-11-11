use rustybuzz::ttf_parser::{RawFace, Tag};

pub(super) fn font_with_empty_table(font: &[u8], tag: Tag) -> Result<Vec<u8>, String> {
    const SFNT_HEADER_BYTES: usize = 12;
    const TABLE_RECORD_BYTES: usize = 16;
    const TABLE_LENGTH_OFFSET: usize = 12;

    let face = RawFace::parse(font, 0).map_err(|error| format!("{error:?}"))?;
    let index = face
        .table_records
        .into_iter()
        .position(|table| table.tag == tag)
        .ok_or("fixture table missing")?;
    let length_offset = SFNT_HEADER_BYTES + index * TABLE_RECORD_BYTES + TABLE_LENGTH_OFFSET;
    let mut data = font.to_vec();
    data[length_offset..length_offset + size_of::<u32>()].copy_from_slice(&0u32.to_be_bytes());
    Ok(data)
}
