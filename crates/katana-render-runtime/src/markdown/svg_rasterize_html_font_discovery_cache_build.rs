use super::super::{CacheValue, DatabaseKey, sources};
#[cfg(target_os = "macos")]
use super::super::{DiscoveryMode, shared_raster::shared_sources::SharedSourceSeed};
use resvg::usvg;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

const MAX_CACHED_IMMUTABLE_FONT_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn build_miss(
    key: &DatabaseKey,
    entries: &VecDeque<(DatabaseKey, CacheValue)>,
) -> CacheValue {
    let budget = MAX_CACHED_IMMUTABLE_FONT_BYTES.saturating_sub(retained_mapping_bytes(
        entries.iter().map(|(_, value)| value),
    ));
    #[cfg(target_os = "macos")]
    let mut candidate = {
        let seeds = if key.discovery == DiscoveryMode::KnownOnly {
            reusable_sources(entries.iter().map(|(_, value)| value))
        } else {
            Vec::new()
        };
        build_with_sources(key, budget, &seeds)
    };
    #[cfg(not(target_os = "macos"))]
    let mut candidate = build(key, budget);
    let retained_and_candidate = entries
        .iter()
        .map(|(_, value)| value)
        .chain(std::iter::once(&candidate));
    if !within_mapping_budget(retained_and_candidate, MAX_CACHED_IMMUTABLE_FONT_BYTES) {
        candidate = build(key, 0);
    }
    candidate
}

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

pub(super) fn within_mapping_budget<'a>(
    values: impl Iterator<Item = &'a CacheValue>,
    limit: usize,
) -> bool {
    retained_mapping_bytes(values) <= limit
}

fn build(key: &DatabaseKey, shared_budget: usize) -> CacheValue {
    #[cfg(target_os = "macos")]
    {
        build_with_sources(key, shared_budget, &[])
    }
    #[cfg(not(target_os = "macos"))]
    {
        build_unshared(key, shared_budget)
    }
}

fn build_unshared(key: &DatabaseKey, shared_budget: usize) -> CacheValue {
    let _ = shared_budget;
    CacheValue {
        database: Arc::new(sources::build_html_font_db(&key.request)),
        #[cfg(target_os = "macos")]
        proof: None,
    }
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

#[cfg(target_os = "macos")]
fn build_with_sources(
    key: &DatabaseKey,
    shared_budget: usize,
    seeds: &[SharedSourceSeed],
) -> CacheValue {
    if key.discovery == DiscoveryMode::KnownOnly {
        let (database, proof) = sources::build_known_html_font_db_with_shared_sources(
            &key.request,
            shared_budget,
            seeds,
        );
        CacheValue {
            database: Arc::new(database),
            proof: Some(Arc::new(proof)),
        }
    } else {
        build_unshared(key, shared_budget)
    }
}
