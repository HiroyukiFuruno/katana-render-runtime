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
            "krr-selector-generation-{}-{}.ttf",
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
    assert_eq!(
        before.durable_reusable(),
        super::super::super::font::file_stamp_durable_reusable(&file.0)
    );
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
    super::super::super::fallback::font_with_only_b_cmap(font)
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
