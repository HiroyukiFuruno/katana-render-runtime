use super::{enable_windows_font_snapshots, windows_font_snapshot, with_validated_stamp_batch};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

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
