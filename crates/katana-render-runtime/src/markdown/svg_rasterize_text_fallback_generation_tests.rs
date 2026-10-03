use super::{FONT, TestResult};
use resvg::usvg::fontdb::{Database, ID, Source};
use std::sync::Arc;

type FallbackFixture = (Arc<Database>, ID, ID, ID);

use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

struct FontFile(std::path::PathBuf);

impl FontFile {
    fn create() -> TestResult<Self> {
        let path = std::env::temp_dir().join(format!(
            "krr-selector-{}-{}.ttf",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, FONT)?;
        Ok(Self(path))
    }
}

impl Drop for FontFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn same_length_same_modified_rewrite_rechecks_fallback_selection() -> TestResult<()> {
    let original = FONT.to_vec();
    let unsupported = font_without_a(&original)?;
    let file = FontFile::create()?;
    std::fs::write(&file.0, &original)?;
    let (database, base, selected, stable) =
        fallback_file_database(&original, &unsupported, &file.0)?;
    let before = super::super::super::font::font_source_generation(&database, selected);
    assert_eq!(before.durable_reusable(), cfg!(unix));
    assert_eq!(select_fallback(&database, base), selected);
    rewrite_same_generation(&file.0, &unsupported)?;
    #[cfg(windows)]
    assert_eq!(
        before,
        super::super::super::font::font_source_generation(&database, selected)
    );
    assert_eq!(select_fallback(&database, base), stable);
    Ok(())
}

fn select_fallback(database: &Arc<Database>, base: ID) -> ID {
    super::super::super::fallback::html_font_runs(database, base, "A", 400, false)[0].0
}

fn fallback_file_database(
    original: &[u8],
    unsupported: &[u8],
    path: &std::path::Path,
) -> TestResult<FallbackFixture> {
    let bundled = crate::markdown::svg_rasterize::font::bundled_font_db();
    let mut face = bundled
        .faces()
        .next()
        .ok_or("bundled font face missing")?
        .clone();
    let mut database = Database::new();
    face.source = Source::Binary(Arc::new(unsupported.to_vec()));
    let base = database.push_face_info(face.clone());
    face.source = Source::File(path.to_path_buf());
    let selected = database.push_face_info(face.clone());
    face.source = Source::Binary(Arc::new(original.to_vec()));
    let stable = database.push_face_info(face);
    Ok((Arc::new(database), base, selected, stable))
}

fn font_without_a(font: &[u8]) -> TestResult<Vec<u8>> {
    let (record, offset) = table_record(font, b"cmap")?;
    let (_, head) = table_record(font, b"head")?;
    let glyph = rustybuzz::ttf_parser::Face::parse(font, 0)
        .map_err(|_| "font invalid")?
        .glyph_index('B')
        .ok_or("B glyph missing")?
        .0;
    let cmap = b_cmap(glyph);
    let mut changed = font.to_vec();
    changed[offset..offset + cmap.len()].copy_from_slice(&cmap);
    changed[record + 4..record + 8].copy_from_slice(&sfnt_checksum(&cmap).to_be_bytes());
    changed[record + 12..record + 16].copy_from_slice(&(cmap.len() as u32).to_be_bytes());
    changed[head + 8..head + 12].fill(0);
    let adjustment = 0xB1B0_AFBA_u32.wrapping_sub(sfnt_checksum(&changed));
    changed[head + 8..head + 12].copy_from_slice(&adjustment.to_be_bytes());
    Ok(changed)
}

fn table_record(font: &[u8], tag: &[u8; 4]) -> TestResult<(usize, usize)> {
    let table_count = usize::from(u16::from_be_bytes([font[4], font[5]]));
    for index in 0..table_count {
        let record = 12 + index * 16;
        let offset = u32::from_be_bytes(font[record + 8..record + 12].try_into()?) as usize;
        if &font[record..record + 4] == tag {
            return Ok((record, offset));
        }
    }
    Err("font table missing".into())
}

fn b_cmap(glyph: u16) -> Vec<u8> {
    let mut cmap = Vec::with_capacity(40);
    cmap.extend_from_slice(&[0, 0, 0, 1, 0, 3, 0, 10, 0, 0, 0, 12]);
    cmap.extend_from_slice(&12_u16.to_be_bytes());
    cmap.extend_from_slice(&0_u16.to_be_bytes());
    cmap.extend_from_slice(&28_u32.to_be_bytes());
    cmap.extend_from_slice(&0_u32.to_be_bytes());
    cmap.extend_from_slice(&1_u32.to_be_bytes());
    cmap.extend_from_slice(&u32::from('B').to_be_bytes());
    cmap.extend_from_slice(&u32::from('B').to_be_bytes());
    cmap.extend_from_slice(&u32::from(glyph).to_be_bytes());
    cmap
}

fn sfnt_checksum(bytes: &[u8]) -> u32 {
    bytes.chunks(4).fold(0_u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

fn rewrite_same_generation(path: &std::path::Path, bytes: &[u8]) -> TestResult<()> {
    let original_len = std::fs::metadata(path)?.len();
    assert_eq!(u64::try_from(bytes.len())?, original_len);
    let modified = std::fs::metadata(path)?.modified()?;
    std::fs::write(path, bytes)?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_times(std::fs::FileTimes::new().set_modified(modified))?;
    Ok(())
}
