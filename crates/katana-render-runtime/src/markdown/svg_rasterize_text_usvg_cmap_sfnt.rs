use skrifa::{FontRef, Tag, charmap::MappingIndex};

pub(super) const CMAP_TAG: Tag = Tag::from_be_bytes(*b"cmap");
const SFNT_HEADER_BYTES: usize = 12;
const TABLE_RECORD_BYTES: usize = 16;
const TABLE_ALIGNMENT: usize = 4;
const CMAP_OFFSET: usize = SFNT_HEADER_BYTES + TABLE_RECORD_BYTES;
const MAX_PADDING_BYTES: usize = TABLE_ALIGNMENT - 1;
const SEARCH_RANGE: u16 = TABLE_RECORD_BYTES as u16;

pub(super) struct CachedMapping {
    pub(super) cmap_sfnt: Vec<u8>,
    pub(super) mapping: MappingIndex,
}

pub(super) fn read_mapping(data: &[u8], face_index: u32) -> Option<CachedMapping> {
    let font = FontRef::from_index(data, face_index).ok()?;
    let table = font.table_data(CMAP_TAG)?;
    let cmap_sfnt = cmap_only_sfnt(table.as_bytes())?;
    let mapping = MappingIndex::new(&font);
    mapping
        .charmap(&font)
        .has_map()
        .then_some(CachedMapping { cmap_sfnt, mapping })
}

pub(super) fn has_character(
    cmap_sfnt: &[u8],
    mapping: MappingIndex,
    character: char,
) -> Option<bool> {
    let font = FontRef::new(cmap_sfnt).ok()?;
    let charmap = mapping.charmap(&font);
    charmap.has_map().then(|| charmap.map(character).is_some())
}

pub(super) fn cmap_only_sfnt(cmap: &[u8]) -> Option<Vec<u8>> {
    let length = u32::try_from(cmap.len()).ok()?;
    let capacity = cmap.len().checked_add(CMAP_OFFSET + MAX_PADDING_BYTES)?;
    if capacity > super::MAX_ENTRY_BYTES {
        return None;
    }
    let mut sfnt = Vec::with_capacity(capacity);
    sfnt.extend_from_slice(&[0, 1, 0, 0, 0, 1]);
    sfnt.extend_from_slice(&SEARCH_RANGE.to_be_bytes());
    sfnt.extend_from_slice(&[0, 0, 0, 0]);
    sfnt.extend_from_slice(b"cmap");
    sfnt.extend_from_slice(&[0, 0, 0, 0]);
    sfnt.extend_from_slice(&(CMAP_OFFSET as u32).to_be_bytes());
    sfnt.extend_from_slice(&length.to_be_bytes());
    sfnt.extend_from_slice(cmap);
    while !sfnt.len().is_multiple_of(TABLE_ALIGNMENT) {
        sfnt.push(0);
    }
    Some(sfnt)
}
