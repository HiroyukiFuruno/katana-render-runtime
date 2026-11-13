use super::{FONT, name_id, ttf_parser, unique_postscript_index};

#[test]
fn bundled_font_without_postscript_records_has_no_unique_identity() -> Result<(), String> {
    let expected_name = "NotoSans-Regular";
    assert_eq!(unique_postscript_index(FONT, expected_name), Some(0));
    let font = with_postscript_records(|record| {
        record[NAME_ID_OFFSET..NAME_ID_OFFSET + size_of::<u16>()]
            .copy_from_slice(&0u16.to_be_bytes())
    })?;
    assert_eq!(unique_postscript_index(&font, expected_name), None);
    Ok(())
}

#[test]
fn bundled_font_with_an_unsupported_postscript_encoding_has_no_unique_identity()
-> Result<(), String> {
    let font = with_postscript_records(|record| {
        record[..2].copy_from_slice(&1u16.to_be_bytes());
        record[size_of::<u16>()..size_of::<[u16; 2]>()].copy_from_slice(&1u16.to_be_bytes());
    })?;
    assert_eq!(unique_postscript_index(&font, "NotoSans-Regular"), None);
    Ok(())
}

fn with_postscript_records(mut change: impl FnMut(&mut [u8])) -> Result<Vec<u8>, String> {
    let mut font = FONT.to_vec();
    let source = read_fonts::FontRef::new(FONT).map_err(|error| error.to_string())?;
    let name = source
        .table_directory()
        .table_records()
        .iter()
        .find(|record| record.tag().to_be_bytes() == *b"name")
        .ok_or("name table missing")?;
    let start = name.offset() as usize + NAME_TABLE_HEADER_BYTES;
    let end = start + postscript_record_count(name.offset() as usize) * NAME_RECORD_BYTES;
    let mut changed = 0;
    for record in font[start..end].chunks_exact_mut(NAME_RECORD_BYTES) {
        let id = u16::from_be_bytes([record[NAME_ID_OFFSET], record[NAME_ID_OFFSET + 1]]);
        if id == name_id::POST_SCRIPT_NAME {
            change(record);
            changed += 1;
        }
    }
    assert!(changed > 0);
    assert!(ttf_parser::Face::parse(&font, 0).is_ok());
    Ok(font)
}

const NAME_TABLE_HEADER_BYTES: usize = 6;
const NAME_RECORD_BYTES: usize = 12;
const NAME_ID_OFFSET: usize = 6;
const NAME_COUNT_OFFSET: usize = 2;

fn postscript_record_count(offset: usize) -> usize {
    let count = offset + NAME_COUNT_OFFSET;
    usize::from(u16::from_be_bytes([FONT[count], FONT[count + 1]]))
}
