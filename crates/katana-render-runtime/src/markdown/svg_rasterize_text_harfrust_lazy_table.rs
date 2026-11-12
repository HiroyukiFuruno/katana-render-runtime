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
    let poisoned = Arc::new(AtomicBool::new(false));
    /* WHY: TableFunctionの構築時には字形数を持つmaxpの有無が検査されない。 */
    let font = directory
        .tables
        .contains_key(b"maxp")
        .then(|| table_source(PathBuf::from(path), directory, Arc::clone(&poisoned)))
        .and_then(|source| Font::new(source, face_index))
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
#[path = "svg_rasterize_text_harfrust_lazy_table_tests.rs"]
mod tests;
