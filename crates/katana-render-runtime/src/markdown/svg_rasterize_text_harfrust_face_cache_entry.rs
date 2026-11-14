use super::super::super::font::FontSourceGeneration;
use super::super::super::font::font_source_generation_uncached;
use super::lazy_table;
use harfrust::font::Font;
use resvg::usvg::fontdb::Database;
use std::{
    ops::Deref,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CacheKey {
    Binary {
        identity: usize,
        len: usize,
        face_index: u32,
    },
    File {
        path: PathBuf,
        generation: FontSourceGeneration,
        face_index: u32,
    },
}

#[derive(Clone)]
pub(crate) struct FontHandle {
    pub(super) font: Font,
    pub(super) key: Option<CacheKey>,
    pub(super) poisoned: Option<Arc<AtomicBool>>,
    pub(super) retained_source_bytes: usize,
}

impl Deref for FontHandle {
    type Target = Font;

    fn deref(&self) -> &Self::Target {
        &self.font
    }
}

impl FontHandle {
    pub(super) fn clean(&self, database: &Database, face_id: resvg::usvg::fontdb::ID) -> bool {
        if self
            .poisoned
            .as_ref()
            .is_some_and(|poisoned| poisoned.load(Ordering::Acquire))
        {
            return false;
        }
        match self.key.as_ref() {
            Some(CacheKey::File { generation, .. }) => {
                font_source_generation_uncached(database, face_id) == *generation
            }
            Some(CacheKey::Binary { .. }) | None => true,
        }
    }
}

pub(super) fn full_file_font(
    database: &Database,
    face_id: resvg::usvg::fontdb::ID,
) -> Option<FontHandle> {
    /* WHY: lazy budget超過時の全source fallbackも、既存handle解放を先に行う。 */
    super::clear_cache();
    database.with_face_data(face_id, |data, index| {
        Some(FontHandle {
            font: Font::new(data.to_vec(), index)?,
            key: None,
            poisoned: None,
            retained_source_bytes: data.len(),
        })
    })?
}

pub(super) fn lazy_file_font(
    database: &Database,
    face_id: resvg::usvg::fontdb::ID,
    path: &Path,
    face_index: u32,
    key: CacheKey,
    generation: FontSourceGeneration,
) -> std::io::Result<Option<FontHandle>> {
    let lazy = lazy_table::load(path, face_index)?;
    let handle = FontHandle {
        font: lazy.font,
        key: Some(key),
        poisoned: Some(lazy.poisoned),
        retained_source_bytes: lazy.retained_source_bytes,
    };
    Ok((font_source_generation_uncached(database, face_id) == generation).then_some(handle))
}

pub(super) fn load_file_font(
    database: &Database,
    face_id: resvg::usvg::fontdb::ID,
    path: &Path,
    face_index: u32,
    key: CacheKey,
    generation: FontSourceGeneration,
    length: u64,
) -> Option<FontHandle> {
    #[cfg(target_os = "macos")]
    match super::immutable_mapping::try_load(
        database,
        face_id,
        path,
        face_index,
        key.clone(),
        generation.clone(),
        length,
    ) {
        Ok(Some(font)) => return Some(font),
        Err(_) => return None,
        Ok(None) => {}
    }
    load_lazy_or_full(database, face_id, path, face_index, key, generation, length)
}

fn load_lazy_or_full(
    database: &Database,
    face_id: resvg::usvg::fontdb::ID,
    path: &Path,
    face_index: u32,
    key: CacheKey,
    generation: FontSourceGeneration,
    length: u64,
) -> Option<FontHandle> {
    if length > MAX_SOURCE_BYTES {
        return full_file_font(database, face_id);
    }
    match lazy_file_font(database, face_id, path, face_index, key, generation) {
        Ok(Some(mut font)) => {
            font.retained_source_bytes = font.retained_source_bytes.max(length as usize);
            Some(font)
        }
        Ok(None) => None,
        Err(error) if error.kind() == std::io::ErrorKind::OutOfMemory => {
            full_file_font(database, face_id)
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::{Arc, AtomicBool, Database, Font, FontHandle, Ordering};

    #[test]
    fn poisoned_handle_rejects_clean_and_fresh_handle_accepts_it() -> Result<(), String> {
        let mut database = Database::new();
        let bytes = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
        database.load_font_data(bytes.to_vec());
        let face = database.faces().next().ok_or("real font face missing")?.id;
        let font = Font::new(bytes.to_vec(), 0).ok_or("real font missing")?;
        let poisoned = Arc::new(AtomicBool::new(false));
        let handle = FontHandle {
            font,
            key: None,
            poisoned: Some(Arc::clone(&poisoned)),
            retained_source_bytes: bytes.len(),
        };
        assert!(handle.clean(&database, face));
        poisoned.store(true, Ordering::Release);
        assert!(!handle.clean(&database, face));
        Ok(())
    }
}

#[cfg(test)]
mod budget_tests {
    use super::{
        CacheKey, Database, FontHandle, FontSourceGeneration, full_file_font, lazy_file_font,
        load_file_font,
    };
    use resvg::usvg::fontdb::{ID, Source};
    use std::{
        io::Write,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
    const TABLE_BUDGET: u32 = 64 * 1024 * 1024;
    const SFNT_HEADER: [u8; 12] = [0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    const SMALL_FILE_BYTES: u32 = 28;
    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
    type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

    struct BoundedFontFile(PathBuf);

    impl BoundedFontFile {
        fn new() -> TestResult<Self> {
            let path = std::env::temp_dir().join(format!(
                "krr-table-budget-{}-{}.ttf",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            let fixture = Self(path);
            file.write_all(FONT_BYTES)?;
            Ok(fixture)
        }
    }

    impl Drop for BoundedFontFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn over_budget_directory() -> Vec<u8> {
        let mut bytes = SFNT_HEADER.to_vec();
        bytes.extend_from_slice(b"cmap");
        bytes.extend_from_slice(&0_u32.to_be_bytes());
        bytes.extend_from_slice(&SMALL_FILE_BYTES.to_be_bytes());
        bytes.extend_from_slice(&(TABLE_BUDGET + 1).to_be_bytes());
        bytes
    }

    fn file_key_and_generation(
        database: &Database,
        face: ID,
        path: &Path,
    ) -> (CacheKey, FontSourceGeneration) {
        let generation =
            super::super::super::super::font::font_source_generation_uncached(database, face);
        let key = CacheKey::File {
            path: path.to_owned(),
            generation: generation.clone(),
            face_index: 0,
        };
        (key, generation)
    }

    fn assert_valid_lazy_source(database: &Database, face: ID, path: &Path) -> TestResult<()> {
        let (key, generation) = file_key_and_generation(database, face, path);
        let lazy = lazy_file_font(database, face, path, 0, key, generation)?
            .ok_or("valid local font did not produce a lazy handle")?;
        assert!(matches!(
            lazy.font.source(),
            harfrust::font::Source::TableFunction(_)
        ));
        assert!(matches!(lazy.key, Some(CacheKey::File { .. })));
        assert!(lazy.poisoned.is_some());
        assert!(assert_full_source(&lazy).is_err());
        Ok(())
    }

    fn assert_full_source(handle: &FontHandle) -> TestResult<()> {
        let harfrust::font::Source::Blob(bytes) = handle.font.source() else {
            return Err("full reader unexpectedly returned a lazy source".into());
        };
        assert_eq!(bytes.as_ref(), over_budget_directory());
        assert!(handle.key.is_none());
        assert!(handle.poisoned.is_none());
        Ok(())
    }

    fn assert_full_fallback(
        database: &Database,
        face: ID,
        path: &Path,
        key: CacheKey,
        generation: FontSourceGeneration,
    ) -> TestResult<()> {
        let expected =
            full_file_font(database, face).ok_or("full reader rejected SFNT directory")?;
        assert_full_source(&expected)?;
        let fallback = load_file_font(
            database,
            face,
            path,
            0,
            key,
            generation,
            u64::from(SMALL_FILE_BYTES),
        )
        .ok_or("table budget fallback rejected full reader source")?;
        assert_full_source(&fallback)
    }

    fn assert_bounded_fallback(database: &Database, face: ID, path: &Path) -> TestResult<()> {
        let (key, generation) = file_key_and_generation(database, face, path);
        let error = lazy_file_font(database, face, path, 0, key.clone(), generation.clone())
            .err()
            .ok_or("table budget input unexpectedly loaded")?;
        assert_eq!(error.kind(), std::io::ErrorKind::OutOfMemory);
        assert_eq!(error.to_string(), "table bytes exceed cache bound");
        assert_eq!(
            database.with_face_data(face, |data, index| (data.to_vec(), index)),
            Some((over_budget_directory(), 0)),
        );
        assert_full_fallback(database, face, path, key, generation)
    }

    #[test]
    fn tiny_over_budget_sfnt_uses_full_read_fallback_without_memory_pressure() -> TestResult<()> {
        let fixture = BoundedFontFile::new()?;
        let mut database = Database::new();
        database.load_font_source(Source::File(fixture.0.clone()));
        let face = database.faces().next().ok_or("real font face missing")?.id;
        assert!(full_file_font(&database, face).is_some());
        assert_valid_lazy_source(&database, face, &fixture.0)?;
        std::fs::write(&fixture.0, over_budget_directory())?;
        assert_eq!(
            std::fs::metadata(&fixture.0)?.len(),
            u64::from(SMALL_FILE_BYTES)
        );
        assert_bounded_fallback(&database, face, &fixture.0)?;
        std::fs::remove_file(&fixture.0)?;
        assert!(full_file_font(&database, face).is_none());
        Ok(())
    }
}
