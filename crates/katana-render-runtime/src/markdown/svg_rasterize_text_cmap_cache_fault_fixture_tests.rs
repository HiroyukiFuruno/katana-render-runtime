use super::fixtures::{
    bundled_bytes, cmap_record, malformed_parsed_cmap, oversized_cmap, valid_font_database,
};
use rustybuzz::ttf_parser::Face;

const TABLE_OFFSET_FIELD: usize = 8;
const U32_BYTE_COUNT: usize = 4;
const U32_LAST_BYTE: usize = 3;

fn font_without_a_mapping(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut font = bytes.to_vec();
    let record = cmap_record(&font)?;
    let offset = read_u32(&font, record + TABLE_OFFSET_FIELD)? as usize;
    write_u16(&mut font, offset + 2, 0)?;
    let face = Face::parse(&font, 0).map_err(|_| "empty-cmap fixture did not parse")?;
    if face.glyph_index('A').is_some() {
        return Err("empty-cmap fixture still maps the target glyph".into());
    }
    Ok(font)
}

#[test]
fn valid_database_rejects_a_valid_face_without_target_glyph() -> Result<(), String> {
    let bytes = font_without_a_mapping(&bundled_bytes()?)?;
    assert!(valid_font_database(&bytes).is_err());
    Ok(())
}

#[test]
fn oversized_fixture_rejects_a_valid_face_without_target_glyph() -> Result<(), String> {
    let bytes = font_without_a_mapping(&bundled_bytes()?)?;
    assert!(oversized_cmap(&bytes, 1).is_err());
    Ok(())
}

#[test]
fn malformed_fixture_rejects_a_valid_face_without_target_glyph() -> Result<(), String> {
    let bytes = font_without_a_mapping(&bundled_bytes()?)?;
    assert!(malformed_parsed_cmap(&bytes).is_err());
    Ok(())
}

#[test]
fn oversized_fixture_rejects_cmap_above_requested_bound() -> Result<(), String> {
    let bytes = bundled_bytes()?;
    assert!(oversized_cmap(&bytes, 0).is_err());
    Ok(())
}

#[test]
fn oversized_fixture_aligns_appended_cmap_for_unaligned_ttf() -> Result<(), String> {
    let mut bytes = bundled_bytes()?;
    bytes.push(0);
    let enlarged = oversized_cmap(&bytes, super::super::storage::MAX_ENTRY_BYTES)?;
    let record = cmap_record(&enlarged)?;
    let offset = read_u32(&enlarged, record + TABLE_OFFSET_FIELD)? as usize;
    assert_eq!(offset % 4, 0);
    let face = Face::parse(&enlarged, 0).map_err(|_| "aligned fixture did not parse")?;
    assert!(face.glyph_index('A').is_some());
    Ok(())
}

#[test]
fn cmap_record_rejects_a_ttf_directory_without_cmap_tag() -> Result<(), String> {
    let mut bytes = bundled_bytes()?;
    let record = cmap_record(&bytes)?;
    let tag = bytes
        .get_mut(record..record + 4)
        .ok_or("fixture cmap directory tag missing")?;
    tag.copy_from_slice(b"CMAP");
    assert!(cmap_record(&bytes).is_err());
    Ok(())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let field = bytes
        .get(offset..offset + U32_BYTE_COUNT)
        .ok_or("fixture field outside the TTF")?;
    Ok(u32::from_be_bytes([
        field[0],
        field[1],
        field[2],
        field[U32_LAST_BYTE],
    ]))
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) -> Result<(), String> {
    let field = bytes
        .get_mut(offset..offset + 2)
        .ok_or("fixture field outside the TTF")?;
    field.copy_from_slice(&value.to_be_bytes());
    Ok(())
}
