use rustybuzz::ttf_parser::{RawFace, Tag};

pub(in crate::markdown::svg_rasterize) fn font_with_empty_table(
    font: &[u8],
    tag: Tag,
) -> Result<Vec<u8>, String> {
    const TABLE_LENGTH_OFFSET: usize = 12;
    let length_offset = font_table_record_offset(font, tag)? + TABLE_LENGTH_OFFSET;
    let mut data = font.to_vec();
    data[length_offset..length_offset + size_of::<u32>()].copy_from_slice(&0u32.to_be_bytes());
    Ok(data)
}

pub(super) fn font_with_renamed_table(
    font: &[u8],
    tag: Tag,
    replacement: Tag,
) -> Result<Vec<u8>, String> {
    let start = font_table_record_offset(font, tag)?;
    let bytes = replacement.to_bytes();
    let mut data = font.to_vec();
    data[start..start + bytes.len()].copy_from_slice(&bytes);
    Ok(data)
}

fn font_table_record_offset(font: &[u8], tag: Tag) -> Result<usize, String> {
    const SFNT_HEADER_BYTES: usize = 12;
    const TABLE_RECORD_BYTES: usize = 16;

    let face = RawFace::parse(font, 0).map_err(|error| format!("{error:?}"))?;
    let index = face
        .table_records
        .into_iter()
        .position(|table| table.tag == tag)
        .ok_or("fixture table missing")?;
    Ok(SFNT_HEADER_BYTES + index * TABLE_RECORD_BYTES)
}
