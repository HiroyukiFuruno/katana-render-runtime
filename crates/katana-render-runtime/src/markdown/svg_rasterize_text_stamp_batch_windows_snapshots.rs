use super::MAX_BATCH_PATHS;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

const MAX_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Default)]
pub(super) struct WindowsFontSnapshots {
    values: HashMap<PathBuf, Arc<Vec<u8>>>,
    unavailable: HashSet<PathBuf>,
    total_bytes: usize,
}

impl WindowsFontSnapshots {
    pub(super) fn read(&mut self, path: &Path) -> Option<Arc<Vec<u8>>> {
        if let Some(bytes) = self.values.get(path) {
            return Some(Arc::clone(bytes));
        }
        if self.unavailable.contains(path)
            || self.values.len() + self.unavailable.len() == MAX_BATCH_PATHS
        {
            return None;
        }
        let remaining = MAX_SNAPSHOT_BYTES.checked_sub(self.total_bytes)?;
        let Some(bytes) = read_regular_font_file(path, remaining) else {
            self.unavailable.insert(path.to_path_buf());
            return None;
        };
        self.total_bytes += bytes.len();
        let bytes = Arc::new(bytes);
        self.values.insert(path.to_path_buf(), Arc::clone(&bytes));
        Some(bytes)
    }

    pub(super) fn matches_current(&self) -> bool {
        self.values
            .iter()
            .all(|(path, expected)| regular_font_file_matches(path, expected))
    }
}

fn read_regular_font_file(path: &Path, remaining: usize) -> Option<Vec<u8>> {
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
    matches!(file.read(&mut trailing), Ok(0)).then_some(bytes)
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
    let mut chunk = [0_u8; 8192];
    for expected_chunk in expected.chunks(chunk.len()) {
        let len = expected_chunk.len();
        if file.read_exact(&mut chunk[..len]).is_err() || &chunk[..len] != expected_chunk {
            return false;
        }
    }
    let mut trailing = [0_u8; 1];
    matches!(file.read(&mut trailing), Ok(0))
}
