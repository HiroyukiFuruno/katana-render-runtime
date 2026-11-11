use harfrust::font::{Blob, Font, Source, TableFunction};
use std::{
    io::{Error, ErrorKind},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[path = "svg_rasterize_text_harfrust_lazy_table_io.rs"]
mod io;
use io::read_table;

#[path = "svg_rasterize_text_harfrust_lazy_table_directory.rs"]
mod directory;
use directory::directory;

pub(super) struct LazyFont {
    pub(super) font: Font,
    pub(super) poisoned: Arc<AtomicBool>,
    pub(super) retained_source_bytes: usize,
}

pub(super) fn load(path: &Path, face_index: u32) -> std::io::Result<LazyFont> {
    let directory = directory(path, face_index)?;
    let retained_source_bytes = directory.file_length.max(
        directory
            .tables
            .values()
            .map(|table| u64::from(table.length))
            .sum::<u64>(),
    ) as usize;
    super::admit_source_budget(retained_source_bytes);
    let path = PathBuf::from(path);
    let poisoned = Arc::new(AtomicBool::new(false));
    let source = table_source(path, directory, Arc::clone(&poisoned));
    let font = Font::new(source, face_index)
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "invalid lazy font"))?;
    Ok(LazyFont {
        font,
        poisoned,
        retained_source_bytes,
    })
}

fn table_source(
    path: PathBuf,
    directory: directory::FaceDirectory,
    poisoned: Arc<AtomicBool>,
) -> Source {
    let callback_poison = Arc::clone(&poisoned);
    Source::TableFunction(TableFunction::new(Arc::new(move |tag| {
        let record = directory.tables.get(&tag.to_be_bytes())?;
        match read_table(&path, directory.file_length, record.offset, record.length) {
            Ok(bytes) => Some(Blob::from(bytes)),
            Err(_) => {
                callback_poison.store(true, Ordering::Release);
                None
            }
        }
    })))
}

#[cfg(test)]
mod tests {
    use super::{Ordering, load};
    use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
    use skrifa::raw::TableProvider;

    #[test]
    fn truncated_font_poisons_an_existing_lazy_table_source()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("font.ttf");
        std::fs::write(&path, FONT_BYTES)?;
        let baseline = load(&path, 0)?;
        assert!(baseline.font.tables().name().is_ok());

        let stale = load(&path, 0)?;
        std::fs::write(&path, [])?;
        assert!(stale.font.tables().name().is_err());
        assert!(stale.poisoned.load(Ordering::Acquire));

        std::fs::write(&path, FONT_BYTES)?;
        let fresh = load(&path, 0)?;
        assert!(fresh.font.tables().name().is_ok());
        assert!(!fresh.poisoned.load(Ordering::Acquire));
        Ok(())
    }
}
