use super::super::file_generation::file_source_stamp;
use super::{
    enable_windows_font_snapshots, memo_usable, windows_font_snapshot,
    windows_font_snapshot_glyph_support, windows_font_snapshot_glyph_support_diagnostics,
    with_validated_stamp_batch,
};
use resvg::usvg::fontdb::{Database, Source};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);
const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

struct Fixture(PathBuf);

impl Fixture {
    fn new(bytes: &[u8]) -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!(
            "krr-windows-font-snapshot-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        Ok(Self(path))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn same_length_rewrite_retries_without_reusing_snapshot_bytes() -> Result<(), String> {
    let fixture = Fixture::new(b"first")?;
    let replacement = b"other";
    let mut attempts = 0;
    let result = with_validated_stamp_batch(|| -> Result<Vec<u8>, String> {
        enable_windows_font_snapshots();
        attempts += 1;
        let bytes = match windows_font_snapshot(&fixture.0) {
            Some(snapshot) => snapshot.as_ref().clone(),
            None => fs::read(&fixture.0).map_err(|error| error.to_string())?,
        };
        if attempts == 1 {
            fs::write(&fixture.0, replacement).map_err(|error| error.to_string())?;
        }
        Ok(bytes)
    });
    assert_eq!(attempts, 2);
    assert_eq!(result?, replacement);
    Ok(())
}

#[test]
fn unavailable_file_uses_the_existing_fresh_path() {
    let path = std::env::temp_dir().join(format!(
        "krr-windows-font-snapshot-missing-{}-{}",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let (first, second) = with_validated_stamp_batch(|| {
        enable_windows_font_snapshots();
        (windows_font_snapshot(&path), windows_font_snapshot(&path))
    });
    assert!(first.is_none());
    assert!(second.is_none());
}

#[test]
fn snapshots_are_released_with_the_render_scope() -> Result<(), String> {
    let fixture = Fixture::new(b"first")?;
    let first = with_validated_stamp_batch(|| {
        enable_windows_font_snapshots();
        windows_font_snapshot(&fixture.0).map(|snapshot| snapshot.as_ref().clone())
    });
    assert_eq!(first, Some(b"first".to_vec()));
    fs::write(&fixture.0, b"other").map_err(|error| error.to_string())?;
    let second = with_validated_stamp_batch(|| {
        enable_windows_font_snapshots();
        windows_font_snapshot(&fixture.0).map(|snapshot| snapshot.as_ref().clone())
    });
    assert_eq!(second, Some(b"other".to_vec()));
    Ok(())
}

#[test]
fn admitted_snapshot_caches_repeated_true_and_false_glyph_support() -> Result<(), String> {
    let fixture = Fixture::new(FONT_BYTES)?;
    let result = with_validated_stamp_batch(|| -> Result<_, String> {
        enable_windows_font_snapshots();
        let first = windows_font_snapshot_glyph_support(&fixture.0, 0, 'A');
        let repeated = windows_font_snapshot_glyph_support(&fixture.0, 0, 'A');
        let absent = windows_font_snapshot_glyph_support(&fixture.0, 0, '\u{10FFFF}');
        let absent_repeated = windows_font_snapshot_glyph_support(&fixture.0, 0, '\u{10FFFF}');
        let diagnostics = windows_font_snapshot_glyph_support_diagnostics()
            .ok_or("glyph support diagnostics are unavailable")?;
        Ok((first, repeated, absent, absent_repeated, diagnostics))
    });
    let (first, repeated, absent, absent_repeated, diagnostics) = result?;

    assert_eq!((first, repeated), (Some(true), Some(true)));
    assert_eq!((absent, absent_repeated), (Some(false), Some(false)));
    assert_eq!(
        (diagnostics.hits, diagnostics.misses, diagnostics.entries),
        (2, 2, 2)
    );
    Ok(())
}

#[test]
fn admitted_snapshot_keeps_generation_nondurable_and_outer_memos_disabled() -> Result<(), String> {
    let fixture = Fixture::new(FONT_BYTES)?;
    let mut database = Database::new();
    database.load_font_source(Source::File(fixture.0.clone()));
    let face = database.faces().next().ok_or("fixture face is missing")?.id;
    let result = with_validated_stamp_batch(|| -> Result<_, String> {
        enable_windows_font_snapshots();
        windows_font_snapshot(&fixture.0).ok_or("fixture snapshot is unavailable")?;
        let (is_file, stamp, generation) = file_source_stamp(&database, face);
        Ok((is_file, stamp, generation.durable_reusable(), memo_usable()))
    });
    let (is_file, stamp, durable_reusable, memo_usable) = result?;

    assert!(is_file);
    assert!(stamp.is_some_and(|value| !value.durable_reusable()));
    assert!(!durable_reusable);
    assert!(!memo_usable);
    Ok(())
}
