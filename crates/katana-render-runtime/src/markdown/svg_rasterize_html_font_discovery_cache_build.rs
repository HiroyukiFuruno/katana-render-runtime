use super::super::{CacheValue, DatabaseKey, sources};
#[cfg(target_os = "macos")]
use super::super::{DiscoveryMode, shared_raster::shared_sources::SharedSourceSeed};
#[cfg(any(target_os = "macos", test))]
use resvg::usvg;
#[cfg(any(target_os = "macos", test))]
use std::collections::HashMap;
use std::{collections::VecDeque, sync::Arc};

#[cfg(target_os = "macos")]
pub(super) const MAX_CACHED_IMMUTABLE_FONT_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn build_miss(
    key: &DatabaseKey,
    entries: &VecDeque<(DatabaseKey, CacheValue)>,
) -> CacheValue {
    #[cfg(target_os = "macos")]
    if key.discovery == DiscoveryMode::KnownOnly {
        /* WHY: 新規mappingは残予算で制限し、seedには既存の共有元だけを渡す。 */
        let retained = entries.iter().map(|(_, value)| value);
        let budget = MAX_CACHED_IMMUTABLE_FONT_BYTES
            .saturating_sub(retained_mapping_bytes(retained.clone()));
        let seeds = reusable_sources(retained);
        let (database, proof) =
            sources::build_known_html_font_db_with_shared_sources(&key.request, budget, &seeds);
        return CacheValue {
            database: Arc::new(database),
            proof: Some(Arc::new(proof)),
        };
    }
    #[cfg(not(target_os = "macos"))]
    let _ = entries;
    CacheValue {
        database: Arc::new(sources::build_html_font_db(&key.request)),
        #[cfg(target_os = "macos")]
        proof: None,
    }
}

#[cfg(any(target_os = "macos", test))]
pub(super) fn retained_mapping_bytes<'a>(values: impl Iterator<Item = &'a CacheValue>) -> usize {
    let mut mappings = HashMap::new();
    for value in values {
        for face in value.database.faces() {
            if let usvg::fontdb::Source::SharedFile(_, data) = &face.source {
                let identity = Arc::as_ptr(data) as *const () as usize;
                mappings
                    .entry(identity)
                    .or_insert_with(|| data.as_ref().as_ref().len());
            }
        }
    }
    mappings
        .values()
        .fold(0_usize, |total, length| total.saturating_add(*length))
}

#[cfg(test)]
pub(super) fn within_mapping_budget<'a>(
    values: impl Iterator<Item = &'a CacheValue>,
    limit: usize,
) -> bool {
    retained_mapping_bytes(values) <= limit
}

#[cfg(target_os = "macos")]
fn reusable_sources<'a>(values: impl Iterator<Item = &'a CacheValue>) -> Vec<SharedSourceSeed> {
    let mut seeds: Vec<SharedSourceSeed> = Vec::new();
    for value in values {
        let Some(proof) = &value.proof else {
            continue;
        };
        for seed in proof.reusable_shared_sources(&value.database) {
            if !seeds
                .iter()
                .any(|(path, stamp, _)| path == &seed.0 && stamp == &seed.1)
            {
                seeds.push(seed);
            }
        }
    }
    seeds
}
