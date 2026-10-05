use super::super::sfnt::{cmap_only_sfnt, has_character};
use skrifa::{FontRef, charmap::MappingIndex};

pub(super) const FORMAT4_SUBTABLE_FORMAT: u16 = 4;
const FORMAT4_SUBTABLE_BYTES: usize = 32;
const FORMAT4_SUBTABLE_LENGTH: u16 = 32;
const FORMAT4_SEGMENT_COUNT_X2: u16 = 4;
const FORMAT4_SEARCH_RANGE: u16 = 4;
const FORMAT4_ENTRY_SELECTOR: u16 = 1;
const FORMAT4_RESERVED_PAD: u16 = 0;
const FORMAT4_ID_RANGE_OFFSET: u16 = 0;
const FORMAT4_END_CODE_SENTINEL: u16 = u16::MAX;
const FORMAT4_GLYPH_ID: i16 = 1;
pub(super) const FORMAT12_SUBTABLE_FORMAT: u16 = 12;
const FORMAT12_HEADER_BYTES: usize = 16;
const FORMAT12_GROUP_BYTES: usize = 12;
const CMAP_HEADER_BYTES: usize = 4;
const CMAP_ENCODING_RECORD_BYTES: usize = 8;
pub(super) const PLATFORM_MICROSOFT: u16 = 3;
const ENCODING_SYMBOL: u16 = 0;
pub(super) const ENCODING_UNICODE_BMP: u16 = 1;
pub(super) const ENCODING_UNICODE_FULL: u16 = 10;
pub(super) const SYMBOL_PRIVATE_USE_A: u16 = 0xF041;
pub(super) const SYMBOL_GLYPH_A: u16 = 7;
pub(super) const UNICODE_BMP_GLYPH_Z: u16 = 7;
const UNICODE_BMP_GLYPH_A: u16 = 2;
const UNICODE_FULL_GLYPH: u32 = 5;
const NOTDEF_GLYPH_ID: u32 = 0;
const FORMAT12_RESERVED: u16 = 0;
const FORMAT12_RESERVED_WORD: u32 = 0;
const CMAP_VERSION: u16 = 0;
const CMAP_ONLY_SFNT_HEADER_BYTES: usize = 28;
const SFNT_MAX_ALIGNMENT_PADDING_BYTES: usize = 3;
const INVALID_SUBTABLE_OFFSET: u16 = u16::MAX;
const MINIMAL_SFNT_HEADER_BYTES: usize = 12;
const SUPPLEMENTARY_TEST_CHARACTER: u32 = 0x1F600;
const SFNT_VERSION_1_0: [u8; 4] = [0, 1, 0, 0];

pub(super) fn format4_mapping(codepoint: u16, glyph_id: u16) -> Vec<u8> {
    let delta = glyph_id.wrapping_sub(codepoint);
    let mut table = Vec::with_capacity(FORMAT4_SUBTABLE_BYTES);
    for value in [
        FORMAT4_SUBTABLE_FORMAT,
        FORMAT4_SUBTABLE_LENGTH,
        FORMAT4_RESERVED_PAD,
        FORMAT4_SEGMENT_COUNT_X2,
        FORMAT4_SEARCH_RANGE,
        FORMAT4_ENTRY_SELECTOR,
        FORMAT4_ID_RANGE_OFFSET,
        codepoint,
        FORMAT4_END_CODE_SENTINEL,
        FORMAT4_RESERVED_PAD,
        codepoint,
        FORMAT4_END_CODE_SENTINEL,
    ] {
        table.extend_from_slice(&value.to_be_bytes());
    }
    table.extend_from_slice(&(delta as i16).to_be_bytes());
    table.extend_from_slice(&FORMAT4_GLYPH_ID.to_be_bytes());
    table.extend_from_slice(&FORMAT4_ID_RANGE_OFFSET.to_be_bytes());
    table.extend_from_slice(&FORMAT4_ID_RANGE_OFFSET.to_be_bytes());
    table
}

fn format12_mapping(groups: &[(u32, u32, u32)]) -> Vec<u8> {
    let length = FORMAT12_HEADER_BYTES + groups.len() * FORMAT12_GROUP_BYTES;
    let mut table = Vec::with_capacity(length);
    table.extend_from_slice(&FORMAT12_SUBTABLE_FORMAT.to_be_bytes());
    table.extend_from_slice(&FORMAT12_RESERVED.to_be_bytes());
    table.extend_from_slice(&(length as u32).to_be_bytes());
    table.extend_from_slice(&FORMAT12_RESERVED_WORD.to_be_bytes());
    table.extend_from_slice(&(groups.len() as u32).to_be_bytes());
    for (start, end, glyph) in groups {
        table.extend_from_slice(&start.to_be_bytes());
        table.extend_from_slice(&end.to_be_bytes());
        table.extend_from_slice(&glyph.to_be_bytes());
    }
    table
}

pub(super) fn cmap_with_records(records: &[(u16, u16, Vec<u8>)]) -> Vec<u8> {
    let header_len = CMAP_HEADER_BYTES + records.len() * CMAP_ENCODING_RECORD_BYTES;
    let mut cmap = Vec::new();
    cmap.extend_from_slice(&CMAP_VERSION.to_be_bytes());
    cmap.extend_from_slice(&(records.len() as u16).to_be_bytes());
    let mut offset = header_len;
    for (platform, encoding, table) in records {
        cmap.extend_from_slice(&platform.to_be_bytes());
        cmap.extend_from_slice(&encoding.to_be_bytes());
        cmap.extend_from_slice(&(offset as u32).to_be_bytes());
        offset += table.len();
    }
    for (_, _, table) in records {
        cmap.extend_from_slice(table);
    }
    cmap
}

fn assert_mapping_matches_skrifa(cmap: &[u8], characters: &[char]) -> Result<(), String> {
    let sfnt = cmap_only_sfnt(cmap).ok_or("cmap-only SFNT wrapper failed")?;
    let font = FontRef::new(&sfnt).map_err(|error| format!("font parse: {error:?}"))?;
    let mapping = MappingIndex::new(&font);
    let stock = skrifa::charmap::Charmap::new(&font);
    for character in characters {
        let expected = stock.map(*character).is_some();
        let expected_result = stock.has_map().then_some(expected);
        assert_eq!(
            has_character(&sfnt, mapping, *character),
            expected_result,
            "cmap mismatch for U+{:04X}",
            *character as u32
        );
    }
    Ok(())
}

#[test]
fn symbol_cmap_preserves_private_use_and_low_byte_aliases() -> Result<(), String> {
    let cmap = cmap_with_records(&[(
        PLATFORM_MICROSOFT,
        ENCODING_SYMBOL,
        format4_mapping(SYMBOL_PRIVATE_USE_A, SYMBOL_GLYPH_A),
    )]);
    assert_mapping_matches_skrifa(&cmap, &['A', 'B', '\u{F041}', '\u{F042}'])?;
    let sfnt = cmap_only_sfnt(&cmap).ok_or("cmap wrapper failed")?;
    let font = FontRef::new(&sfnt).map_err(|error| format!("font parse: {error:?}"))?;
    let charmap = skrifa::charmap::Charmap::new(&font);
    assert!(charmap.is_symbol());
    assert!(charmap.map('A').is_some());
    assert!(charmap.map('\u{F041}').is_some());
    Ok(())
}

#[test]
fn unicode_full_mapping_wins_bmp_regardless_of_record_order() -> Result<(), String> {
    let bmp = (
        PLATFORM_MICROSOFT,
        ENCODING_UNICODE_BMP,
        format4_mapping('A' as u16, UNICODE_BMP_GLYPH_A),
    );
    let full = (
        PLATFORM_MICROSOFT,
        ENCODING_UNICODE_FULL,
        format12_mapping(&[
            ('A' as u32, 'A' as u32, NOTDEF_GLYPH_ID),
            (
                SUPPLEMENTARY_TEST_CHARACTER,
                SUPPLEMENTARY_TEST_CHARACTER,
                UNICODE_FULL_GLYPH,
            ),
        ]),
    );
    for records in [[bmp.clone(), full.clone()], [full.clone(), bmp.clone()]] {
        let cmap = cmap_with_records(&records);
        assert_mapping_matches_skrifa(&cmap, &['A', '\u{1F600}', 'B'])?;
        let sfnt = cmap_only_sfnt(&cmap).ok_or("cmap wrapper failed")?;
        let font = FontRef::new(&sfnt).map_err(|error| format!("font parse: {error:?}"))?;
        let charmap = skrifa::charmap::Charmap::new(&font);
        assert_eq!(charmap.map('A'), None, "glyph zero must be absent");
        assert!(charmap.map('\u{1F600}').is_some());
    }
    Ok(())
}

#[test]
fn missing_and_malformed_cmap_tables_do_not_create_false_support() -> Result<(), String> {
    let mut missing_sfnt = vec![0_u8; MINIMAL_SFNT_HEADER_BYTES];
    missing_sfnt[..SFNT_VERSION_1_0.len()].copy_from_slice(&SFNT_VERSION_1_0);
    let missing_font =
        FontRef::new(&missing_sfnt).map_err(|error| format!("font parse: {error:?}"))?;
    assert!(!skrifa::charmap::Charmap::new(&missing_font).has_map());
    let missing_mapping = MappingIndex::new(&missing_font);
    assert_eq!(has_character(&missing_sfnt, missing_mapping, 'A'), None);

    let malformed = malformed_cmap_with_invalid_subtable_offset();
    assert_mapping_matches_skrifa(&malformed, &['A', '\u{1F600}'])
}

fn malformed_cmap_with_invalid_subtable_offset() -> Vec<u8> {
    let mut cmap = Vec::with_capacity(CMAP_HEADER_BYTES + CMAP_ENCODING_RECORD_BYTES);
    cmap.extend_from_slice(&CMAP_VERSION.to_be_bytes());
    cmap.extend_from_slice(&1_u16.to_be_bytes());
    cmap.extend_from_slice(&PLATFORM_MICROSOFT.to_be_bytes());
    cmap.extend_from_slice(&ENCODING_UNICODE_BMP.to_be_bytes());
    cmap.extend_from_slice(&(INVALID_SUBTABLE_OFFSET as u32).to_be_bytes());
    cmap
}

#[test]
fn cmap_sfnt_wrapper_rejects_oversized_input_before_allocation() {
    let wrapper_overhead = CMAP_ONLY_SFNT_HEADER_BYTES + SFNT_MAX_ALIGNMENT_PADDING_BYTES;
    let max_cmap = super::super::MAX_ENTRY_BYTES - wrapper_overhead;
    let accepted = super::super::sfnt::cmap_only_sfnt(&vec![0; max_cmap]);
    assert!(accepted.is_some_and(|sfnt| sfnt.len() <= super::super::MAX_ENTRY_BYTES));
    assert!(super::super::sfnt::cmap_only_sfnt(&vec![0; max_cmap + 1]).is_none());
}
