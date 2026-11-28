use super::{SharedSources, stamp_path_uncached};
use crate::markdown::svg_rasterize::font::html_font_db_for_text;
use resvg::usvg::fontdb::Source;
use std::{io::Write, path::PathBuf};

const SENTINEL: u8 = 0x5a;

struct CopiedFont {
    directory: PathBuf,
    path: PathBuf,
}

impl CopiedFont {
    fn new() -> Result<Self, String> {
        let database = html_font_db_for_text("Arial", "source stamp");
        let source = database
            .faces()
            .find_map(|face| match &face.source {
                Source::File(path) => Some(path),
                _ => None,
            })
            .ok_or("real File source missing")?;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "krr-shared-source-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).map_err(|error| error.to_string())?;
        let copied = Self {
            path: directory.join("real-font.ttf"),
            directory,
        };
        std::fs::copy(source, &copied.path).map_err(|error| error.to_string())?;
        Ok(copied)
    }
}

impl Drop for CopiedFont {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn source_id_reuses_one_arc_only_for_the_same_path_and_stamp() -> Result<(), String> {
    let copied = CopiedFont::new()?;
    let stamp = stamp_path_uncached(&copied.path).map_err(|_| "stamp failed")?;
    let mut sources = SharedSources::default();
    let first = sources
        .get(&copied.path, &stamp)
        .ok_or("first map failed")?;
    let second = sources
        .get(&copied.path, &stamp)
        .ok_or("cached map failed")?;
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert_eq!(sources.paths.len(), 1);
    Ok(())
}

#[test]
fn changed_stamp_cannot_reuse_an_old_source_id_or_blob() -> Result<(), String> {
    let copied = CopiedFont::new()?;
    let before = stamp_path_uncached(&copied.path).map_err(|_| "stamp failed")?;
    let mut sources = SharedSources::default();
    let original = sources
        .get(&copied.path, &before)
        .ok_or("first map failed")?;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&copied.path)
        .and_then(|mut file| file.write_all(&[SENTINEL]))
        .map_err(|error| error.to_string())?;
    let after = stamp_path_uncached(&copied.path).map_err(|_| "changed stamp failed")?;
    assert_ne!(before, after);
    assert!(sources.get(&copied.path, &before).is_none());
    assert!(sources.get(&copied.path, &after).is_none());
    let fresh = SharedSources::default()
        .get(&copied.path, &after)
        .ok_or("fresh map failed")?;
    assert!(!std::sync::Arc::ptr_eq(&original, &fresh));
    Ok(())
}

#[path = "svg_rasterize_html_font_shared_source_fault_tests.rs"]
mod fault_tests;
