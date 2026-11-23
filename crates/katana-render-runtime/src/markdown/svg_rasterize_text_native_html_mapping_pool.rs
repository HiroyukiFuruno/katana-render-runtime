use super::super::super::super::harfrust::map_immutable_font_data;
use super::{Arc, FileStamp, MappedFontBytes, Path, PathBuf};
use std::sync::{Mutex, Weak};

const MAX_TRACKED_MAPPINGS: usize = 32;

struct Mapping {
    path: PathBuf,
    stamp: FileStamp,
    bytes: Weak<dyn AsRef<[u8]> + Send + Sync>,
}

static MAPPINGS: Mutex<Vec<Mapping>> = Mutex::new(Vec::new());

pub(super) fn acquire(path: &Path, stamp: &FileStamp, length: u64) -> Option<MappedFontBytes> {
    let mut mappings = MAPPINGS.lock().unwrap_or_else(|error| error.into_inner());
    mappings.retain(|mapping| mapping.bytes.strong_count() != 0);
    if let Some(bytes) = lookup(&mappings, path, stamp) {
        return Some(bytes);
    }
    let bytes = map_immutable_font_data(path, length).ok()?;
    /* WHY: mappingはセッションだけが保有し、共有用の参照は最新の32件に制限する。 */
    let excess = (mappings.len() + 1).saturating_sub(MAX_TRACKED_MAPPINGS);
    mappings.drain(..excess);
    mappings.push(Mapping {
        path: path.to_owned(),
        stamp: stamp.clone(),
        bytes: Arc::downgrade(&bytes),
    });
    Some(bytes)
}

fn lookup(mappings: &[Mapping], path: &Path, stamp: &FileStamp) -> Option<MappedFontBytes> {
    mappings
        .iter()
        .find(|mapping| mapping.path == path && &mapping.stamp == stamp)?
        .bytes
        .upgrade()
}
