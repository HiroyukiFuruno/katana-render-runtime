use super::cmap_directory;
use crate::markdown::svg_rasterize::text_shaping::fallback::test_support::{
    SFNT_CHECKSUM_WORD_BYTES, b_cmap, sfnt_checksum,
};

const SFNT_TABLE_CHECKSUM_OFFSET: usize = 4;
const SFNT_TABLE_LENGTH_OFFSET: usize = 12;
const HEAD_CHECKSUM_ADJUSTMENT_OFFSET: usize = 8;
const SFNT_CHECKSUM_MAGIC: u32 = 0xB1B0_AFBA;

pub(super) fn font_with_only_b_cmap(font: &[u8]) -> Result<Vec<u8>, String> {
    let (record, offset, head) = cmap_directory(font)?;
    let glyph = rustybuzz::ttf_parser::Face::parse(font, 0)
        .map_err(|_| "font invalid")?
        .glyph_index('B')
        .ok_or("B glyph missing")?
        .0;
    let mut changed = font.to_vec();
    rewrite_cmap(&mut changed, record, offset, head, &b_cmap(glyph))?;
    Ok(changed)
}

fn rewrite_cmap(
    changed: &mut [u8],
    record: usize,
    offset: usize,
    head: usize,
    cmap: &[u8],
) -> Result<(), String> {
    changed[offset..offset + cmap.len()].copy_from_slice(cmap);
    changed[record + SFNT_TABLE_CHECKSUM_OFFSET
        ..record + SFNT_TABLE_CHECKSUM_OFFSET + SFNT_CHECKSUM_WORD_BYTES]
        .copy_from_slice(&sfnt_checksum(cmap).to_be_bytes());
    let cmap_len = u32::try_from(cmap.len())
        .map_err(|_| "cmap too large")?
        .to_be_bytes();
    changed[record + SFNT_TABLE_LENGTH_OFFSET
        ..record + SFNT_TABLE_LENGTH_OFFSET + SFNT_CHECKSUM_WORD_BYTES]
        .copy_from_slice(&cmap_len);
    changed[head + HEAD_CHECKSUM_ADJUSTMENT_OFFSET
        ..head + HEAD_CHECKSUM_ADJUSTMENT_OFFSET + SFNT_CHECKSUM_WORD_BYTES]
        .fill(0);
    let adjustment = SFNT_CHECKSUM_MAGIC.wrapping_sub(sfnt_checksum(changed));
    changed[head + HEAD_CHECKSUM_ADJUSTMENT_OFFSET
        ..head + HEAD_CHECKSUM_ADJUSTMENT_OFFSET + SFNT_CHECKSUM_WORD_BYTES]
        .copy_from_slice(&adjustment.to_be_bytes());
    Ok(())
}

pub(super) fn rewrite_with_same_modified_time(
    path: &std::path::Path,
    bytes: &[u8],
) -> std::io::Result<()> {
    let modified = std::fs::metadata(path)?.modified()?;
    std::fs::write(path, bytes)?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_times(std::fs::FileTimes::new().set_modified(modified))
}
