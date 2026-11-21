use super::super::super::super::text_shaping::{FileStamp, stamp_path_uncached};
use fontique::{SourceCache, SourceId, SourceInfo, SourceKind};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

pub(in super::super) type SharedBytes = Arc<dyn AsRef<[u8]> + Send + Sync>;
pub(in super::super) type SharedSourceSeed = (PathBuf, FileStamp, SharedBytes);

struct Mapping {
    stamp: FileStamp,
    source: SourceInfo,
}

#[derive(Default)]
pub(in super::super) struct SharedSources {
    cache: SourceCache,
    paths: BTreeMap<PathBuf, Mapping>,
    seeded: Vec<SharedSourceSeed>,
}

impl SharedSources {
    pub(in super::super) fn with_seeds(seeded: &[SharedSourceSeed]) -> Self {
        Self {
            seeded: seeded.to_vec(),
            ..Self::default()
        }
    }

    pub(in super::super) fn seeded(&self, path: &Path, stamp: &FileStamp) -> Option<&SharedBytes> {
        self.seeded
            .iter()
            .find(|(seed_path, seed_stamp, _)| seed_path == path && seed_stamp == stamp)
            .map(|(_, _, data)| data)
    }

    pub(in super::super) fn get(&mut self, path: &Path, stamp: &FileStamp) -> Option<SharedBytes> {
        #[cfg(test)]
        super::super::raster_observer::count(super::super::raster_observer::Metric::SourceGet);
        if !stamp.durable_reusable() || stamp_path_uncached(path).as_ref() != Ok(stamp) {
            return None;
        }
        #[cfg(test)]
        let first = std::cell::Cell::new(false);
        let mapping = self.paths.entry(path.to_path_buf()).or_insert_with(|| {
            #[cfg(test)]
            first.set(true);
            Mapping {
                stamp: stamp.clone(),
                source: SourceInfo::new(SourceId::new(), SourceKind::Path(Arc::from(path))),
            }
        });
        if mapping.stamp != *stamp {
            return None;
        }
        #[cfg(test)]
        observe_get(path, stamp, first.get());
        let data = self.cache.get(&mapping.source)?.into_raw_parts().0;
        #[cfg(test)]
        if first.get() {
            super::super::raster_observer::count(
                super::super::raster_observer::Metric::SourceFirstLoadSuccess,
            );
        }
        validate_acquired_source(path, stamp, data)
    }
}

pub(in super::super) fn validate_acquired_source(
    path: &Path,
    stamp: &FileStamp,
    data: SharedBytes,
) -> Option<SharedBytes> {
    (stamp_path_uncached(path).as_ref() == Ok(stamp)).then_some(data)
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_shared_sources_tests.rs"]
mod tests;

#[cfg(test)]
fn observe_get(path: &Path, stamp: &FileStamp, first: bool) {
    super::super::super::super::text_shaping::observe_source_cache_get(path, stamp);
    if first {
        super::super::raster_observer::source_open(path, stamp);
    } else {
        super::super::raster_observer::count(
            super::super::raster_observer::Metric::SourceOccupiedGet,
        );
    }
}
