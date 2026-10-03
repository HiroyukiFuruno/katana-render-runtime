use resvg::usvg::fontdb::{Database, Source};
use rustybuzz::ttf_parser::{Face, Tag};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

static TEMP_FILE: AtomicUsize = AtomicUsize::new(0);
const CMAP_TAG: Tag = Tag::from_bytes(b"cmap");

pub(super) struct TempFile(pub(super) std::path::PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub(super) struct FileFixture {
    pub(super) database: Arc<Database>,
    pub(super) face_id: resvg::usvg::fontdb::ID,
    pub(super) file: TempFile,
}

pub(super) fn file_database(bytes: &[u8]) -> Result<FileFixture, String> {
    let face = crate::markdown::svg_rasterize::font::bundled_font_db()
        .faces()
        .next()
        .ok_or("bundled face missing")?
        .clone();
    let path = std::env::temp_dir().join(format!(
        "krr-cmap-fault-{}-{}.ttf",
        std::process::id(),
        TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, bytes)
        .as_ref()
        .map_err(ToString::to_string)?;
    let mut face = face;
    face.source = Source::File(path.clone());
    let mut database = Database::new();
    let face_id = database.push_face_info(face);
    Ok(FileFixture {
        database: Arc::new(database),
        face_id,
        file: TempFile(path),
    })
}

pub(super) fn valid_font_database(bytes: &[u8]) -> Result<FileFixture, String> {
    let face = Face::parse(bytes, 0).map_err(|_| "fixture is not a valid TTF face")?;
    if face.glyph_index('A').is_none() {
        return Err("fixture face does not map the expected glyph".into());
    }
    file_database(bytes)
}

pub(super) fn bundled_bytes() -> Result<Vec<u8>, String> {
    let database = crate::markdown::svg_rasterize::font::bundled_font_db();
    let face = database.faces().next().ok_or("bundled face missing")?;
    database
        .with_face_data(face.id, |data, _| data.to_vec())
        .ok_or("bundled bytes missing".into())
}

pub(super) fn oversized_cmap(bytes: &[u8], max_entry_bytes: usize) -> Result<Vec<u8>, String> {
    let original_face = Face::parse(bytes, 0).map_err(|_| "original TTF face is invalid")?;
    let original_glyph = original_face.glyph_index('A');
    if original_glyph.is_none() {
        return Err("original fixture does not map the expected glyph".into());
    }
    let record = cmap_record(bytes)?;
    let (original_table, target_len) = cmap_table_to_enlarge(bytes, record, max_entry_bytes)?;
    let enlarged = append_oversized_cmap(bytes, record, original_table, target_len)?;
    validate_oversized_cmap(&enlarged, original_glyph)?;
    Ok(enlarged)
}

fn cmap_table_to_enlarge(
    bytes: &[u8],
    record: usize,
    max_entry_bytes: usize,
) -> Result<(&[u8], usize), String> {
    let original_offset = read_u32(bytes, record + 8)? as usize;
    let original_len = read_u32(bytes, record + 12)? as usize;
    let original_end = original_offset
        .checked_add(original_len)
        .ok_or("original cmap range overflow")?;
    let original_table = bytes
        .get(original_offset..original_end)
        .ok_or("original cmap table is outside the TTF")?;
    let target_len = max_entry_bytes.checked_add(1).ok_or("cmap size overflow")?;
    if original_table.len() > target_len {
        return Err("original cmap unexpectedly exceeds the cache limit".into());
    }
    Ok((original_table, target_len))
}

fn append_oversized_cmap(
    bytes: &[u8],
    record: usize,
    original_table: &[u8],
    target_len: usize,
) -> Result<Vec<u8>, String> {
    let mut enlarged = bytes.to_vec();
    while !enlarged.len().is_multiple_of(4) {
        enlarged.push(0);
    }
    let new_offset = enlarged.len();
    enlarged.extend_from_slice(original_table);
    enlarged.resize(
        new_offset
            .checked_add(target_len)
            .ok_or("enlarged cmap range overflow")?,
        0,
    );
    let new_offset =
        u32::try_from(new_offset).map_err(|_| "enlarged cmap offset exceeds TTF range")?;
    let target_len =
        u32::try_from(target_len).map_err(|_| "enlarged cmap length exceeds TTF range")?;
    write_u32(&mut enlarged, record + 8, new_offset)?;
    write_u32(&mut enlarged, record + 12, target_len)?;
    Ok(enlarged)
}

fn validate_oversized_cmap(
    enlarged: &[u8],
    original_glyph: Option<rustybuzz::ttf_parser::GlyphId>,
) -> Result<(), String> {
    let enlarged_face = Face::parse(enlarged, 0).map_err(|_| "enlarged TTF face is invalid")?;
    assert_eq!(enlarged_face.glyph_index('A'), original_glyph);
    assert!(enlarged_face.raw_face().table(CMAP_TAG).is_some());
    Ok(())
}

pub(super) fn malformed_parsed_cmap(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let original_face = Face::parse(bytes, 0).map_err(|_| "original TTF face is invalid")?;
    if original_face.glyph_index('A').is_none() {
        return Err("original fixture does not map the expected glyph".into());
    }
    let mut malformed = bytes.to_vec();
    let record = cmap_record(&malformed)?;
    write_u32(&mut malformed, record + 12, 2)?;

    let malformed_face =
        Face::parse(&malformed, 0).map_err(|_| "malformed-cmap TTF face did not parse")?;
    assert!(malformed_face.tables().cmap.is_none());
    assert!(malformed_face.raw_face().table(CMAP_TAG).is_some());
    assert!(malformed_face.glyph_index('A').is_none());
    Ok(malformed)
}

pub(super) fn cmap_record(bytes: &[u8]) -> Result<usize, String> {
    let table_count = read_u16(bytes, 4)? as usize;
    (0..table_count)
        .map(|index| 12 + index * 16)
        .find(|record| {
            bytes
                .get(*record..*record + 4)
                .is_some_and(|tag| tag == b"cmap")
        })
        .ok_or_else(|| "cmap directory record missing".into())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or("16-bit field outside the TTF")?;
    Ok(u16::from_be_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or("32-bit field outside the TTF")?;
    Ok(u32::from_be_bytes([value[0], value[1], value[2], value[3]]))
}

pub(super) fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), String> {
    let field = bytes
        .get_mut(offset..offset + 4)
        .ok_or("32-bit field outside the TTF")?;
    field.copy_from_slice(&value.to_be_bytes());
    Ok(())
}
