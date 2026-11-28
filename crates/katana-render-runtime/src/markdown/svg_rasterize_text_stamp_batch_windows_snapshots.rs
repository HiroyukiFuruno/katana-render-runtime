use super::super::file_generation::{FileStamp, non_durable_file_stamp};
use super::MAX_BATCH_PATHS;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

const MAX_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;
const VALIDATION_CHUNK_BYTES: usize = 8192;
const MAX_GLYPH_SUPPORT_ENTRIES: usize = 65_536;

#[derive(Eq, Hash, PartialEq)]
struct GlyphSupportKey {
    snapshot_identity: usize,
    face_index: u32,
    character: char,
}

struct SnapshotValue {
    bytes: Arc<Vec<u8>>,
    stamp: FileStamp,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in super::super) struct GlyphSupportDiagnostics {
    pub(in super::super) hits: usize,
    pub(in super::super) misses: usize,
    pub(in super::super) entries: usize,
}

#[derive(Default)]
pub(super) struct WindowsFontSnapshots {
    values: HashMap<PathBuf, SnapshotValue>,
    unavailable: HashSet<PathBuf>,
    glyph_support: HashMap<GlyphSupportKey, bool>,
    #[cfg(test)]
    glyph_support_diagnostics: GlyphSupportDiagnostics,
    total_bytes: usize,
}

impl WindowsFontSnapshots {
    pub(super) fn read(&mut self, path: &Path) -> Option<Arc<Vec<u8>>> {
        if let Some(snapshot) = self.values.get(path) {
            return Some(Arc::clone(&snapshot.bytes));
        }
        if self.unavailable.contains(path)
            || self.values.len() + self.unavailable.len() == MAX_BATCH_PATHS
        {
            return None;
        }
        let remaining = MAX_SNAPSHOT_BYTES.checked_sub(self.total_bytes)?;
        let Some((bytes, stamp)) = read_regular_font_file(path, remaining) else {
            self.unavailable.insert(path.to_path_buf());
            return None;
        };
        self.total_bytes += bytes.len();
        let bytes = Arc::new(bytes);
        self.values.insert(
            path.to_path_buf(),
            SnapshotValue {
                bytes: Arc::clone(&bytes),
                stamp,
            },
        );
        Some(bytes)
    }

    pub(super) fn matches_current(&self) -> bool {
        self.values
            .iter()
            .all(|(path, expected)| regular_font_file_matches(path, &expected.bytes))
    }

    pub(super) fn stamp(&self, path: &Path) -> Option<FileStamp> {
        self.values.get(path).map(|snapshot| snapshot.stamp.clone())
    }

    pub(super) fn glyph_support(
        &mut self,
        path: &Path,
        face_index: u32,
        character: char,
    ) -> Option<bool> {
        let bytes = self.read(path)?;
        let key = GlyphSupportKey {
            snapshot_identity: Arc::as_ptr(&bytes) as usize,
            face_index,
            character,
        };
        if let Some(supports) = self.glyph_support.get(&key).copied() {
            self.record_glyph_support_hit();
            return Some(supports);
        }
        let supports = rustybuzz::ttf_parser::Face::parse(bytes.as_slice(), face_index)
            .ok()
            .map(|face| face.glyph_index(character).is_some())?;
        if self.glyph_support.len() < MAX_GLYPH_SUPPORT_ENTRIES {
            self.glyph_support.insert(key, supports);
        }
        self.record_glyph_support_miss();
        Some(supports)
    }

    #[cfg(test)]
    pub(super) fn glyph_support_diagnostics(&self) -> GlyphSupportDiagnostics {
        self.glyph_support_diagnostics
    }

    fn record_glyph_support_hit(&mut self) {
        #[cfg(test)]
        {
            self.glyph_support_diagnostics.hits += 1;
        }
    }

    fn record_glyph_support_miss(&mut self) {
        #[cfg(test)]
        {
            self.glyph_support_diagnostics.misses += 1;
            self.glyph_support_diagnostics.entries = self.glyph_support.len();
        }
    }
}

fn read_regular_font_file(path: &Path, remaining: usize) -> Option<(Vec<u8>, FileStamp)> {
    let mut file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }
    let expected_len = usize::try_from(metadata.len()).ok()?;
    if expected_len > remaining {
        return None;
    }
    let mut bytes = vec![0_u8; expected_len];
    file.read_exact(&mut bytes).ok()?;
    let mut trailing = [0_u8; 1];
    matches!(file.read(&mut trailing), Ok(0)).then_some((bytes, non_durable_file_stamp(&metadata)))
}

fn regular_font_file_matches(path: &Path, expected: &[u8]) -> bool {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let metadata = match file.metadata() {
        Ok(metadata) if metadata.is_file() => metadata,
        _ => return false,
    };
    if usize::try_from(metadata.len()).ok() != Some(expected.len()) {
        return false;
    }
    let mut chunk = [0_u8; VALIDATION_CHUNK_BYTES];
    for expected_chunk in expected.chunks(chunk.len()) {
        let len = expected_chunk.len();
        if file.read_exact(&mut chunk[..len]).is_err() || &chunk[..len] != expected_chunk {
            return false;
        }
    }
    let mut trailing = [0_u8; 1];
    matches!(file.read(&mut trailing), Ok(0))
}
