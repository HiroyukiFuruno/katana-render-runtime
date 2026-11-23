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

#[cfg(test)]
mod tests {
    use super::super::observe_path_stamp;
    use super::{MAPPINGS, acquire};
    use crate::system::ProcessService;
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

    const CASE: &str = "markdown::svg_rasterize::text_shaping::native_html_runs::memo::files::mapping_pool::tests::poisoned_mapping_pool_recovers_and_rejects_failed_mapping";
    const STAMP_BYTES: &[u8] = b"stamp";

    #[test]
    fn poisoned_mapping_pool_recovers_and_rejects_failed_mapping()
    -> Result<(), Box<dyn std::error::Error>> {
        if !ProcessService::enter_isolated_test_child("KRR_NATIVE_HTML_MAPPING_POOL_CHILD", CASE)? {
            return Ok(());
        }

        let guard = MAPPINGS
            .lock()
            .ok()
            .ok_or("mapping pool was already poisoned")?;
        let interrupted = catch_unwind(AssertUnwindSafe(move || {
            let _guard = guard;
            resume_unwind(Box::new("interrupt mapping pool operation"));
        }));
        assert!(interrupted.is_err());
        assert!(MAPPINGS.is_poisoned());

        let fixture = tempfile::tempdir()?;
        let stamp_source = fixture.path().join("stamp-source.ttf");
        std::fs::write(&stamp_source, STAMP_BYTES)?;
        let stamp = observe_path_stamp(&stamp_source)
            .ok()
            .ok_or("fixture stamp missing")?;
        let missing = fixture.path().join("missing.ttf");
        assert!(acquire(&missing, &stamp, 0).is_none());
        let mapped = acquire(&stamp_source, &stamp, STAMP_BYTES.len() as u64)
            .ok_or("valid bytes were not mapped after the failed attempt")?;
        assert_eq!(mapped.as_ref().as_ref(), STAMP_BYTES);
        Ok(())
    }
}
