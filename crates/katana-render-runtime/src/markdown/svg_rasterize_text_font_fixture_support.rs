pub(in super::super) const SFNT_CHECKSUM_WORD_BYTES: usize = 4;
const CMAP_DIRECTORY_HEADER_BYTES: usize = 4;
const CMAP_ENCODING_RECORD_BYTES: usize = 8;
const CMAP_FORMAT_12_HEADER_BYTES: usize = 16;
const CMAP_FORMAT_12_GROUP_BYTES: usize = 12;
const CMAP_FORMAT_12_SUBTABLE_OFFSET: u32 = 12;
const CMAP_TABLE_COUNT: u16 = 1;
const CMAP_PLATFORM_ID_WINDOWS: u16 = 3;
const CMAP_ENCODING_ID_UNICODE_FULL_REPERTOIRE: u16 = 10;
const CMAP_FORMAT_12: u16 = 12;
const CMAP_FORMAT_12_LENGTH: u32 = 28;

pub(in super::super) fn b_cmap(glyph: u16) -> Vec<u8> {
    let mut cmap = Vec::with_capacity(
        CMAP_DIRECTORY_HEADER_BYTES
            + CMAP_ENCODING_RECORD_BYTES
            + CMAP_FORMAT_12_HEADER_BYTES
            + CMAP_FORMAT_12_GROUP_BYTES,
    );
    cmap.extend_from_slice(&0_u16.to_be_bytes());
    cmap.extend_from_slice(&CMAP_TABLE_COUNT.to_be_bytes());
    cmap.extend_from_slice(&CMAP_PLATFORM_ID_WINDOWS.to_be_bytes());
    cmap.extend_from_slice(&CMAP_ENCODING_ID_UNICODE_FULL_REPERTOIRE.to_be_bytes());
    cmap.extend_from_slice(&CMAP_FORMAT_12_SUBTABLE_OFFSET.to_be_bytes());
    cmap.extend_from_slice(&CMAP_FORMAT_12.to_be_bytes());
    cmap.extend_from_slice(&0_u16.to_be_bytes());
    cmap.extend_from_slice(&CMAP_FORMAT_12_LENGTH.to_be_bytes());
    cmap.extend_from_slice(&0_u32.to_be_bytes());
    cmap.extend_from_slice(&1_u32.to_be_bytes());
    cmap.extend_from_slice(&u32::from('B').to_be_bytes());
    cmap.extend_from_slice(&u32::from('B').to_be_bytes());
    cmap.extend_from_slice(&u32::from(glyph).to_be_bytes());
    cmap
}

pub(in super::super) fn sfnt_checksum(bytes: &[u8]) -> u32 {
    bytes
        .chunks(SFNT_CHECKSUM_WORD_BYTES)
        .fold(0_u32, |sum, chunk| {
            let mut word = [0; SFNT_CHECKSUM_WORD_BYTES];
            word[..chunk.len()].copy_from_slice(chunk);
            sum.wrapping_add(u32::from_be_bytes(word))
        })
}

pub(in super::super) fn write_file_with_new_modified_time(
    path: &std::path::Path,
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let before = std::fs::metadata(path)?;
    let replacement_len = u64::try_from(bytes.len())?;
    assert_eq!(before.len(), replacement_len);
    let before_modified = before.modified()?;
    std::fs::write(path, bytes)?;
    const TEST_MODIFIED_TIME_ADVANCE_SECONDS: u64 = 1;
    let modified =
        before_modified + std::time::Duration::from_secs(TEST_MODIFIED_TIME_ADVANCE_SECONDS);
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_times(std::fs::FileTimes::new().set_modified(modified))?;
    assert_ne!(std::fs::metadata(path)?.modified()?, before_modified);
    Ok(())
}
