#[cfg(target_os = "macos")]
use super::super::shared_raster::shared_sources::SharedBytes;
use super::{CacheValue, retained_mapping_bytes, within_mapping_budget};
#[cfg(target_os = "macos")]
use super::{DatabaseKey, HtmlFontDatabaseCache, HtmlFontRequest, sources};
#[cfg(target_os = "macos")]
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use resvg::usvg::fontdb::{Database, Source};
#[cfg(target_os = "macos")]
use std::collections::VecDeque;
use std::{path::PathBuf, sync::Arc};

fn value_with_source(source: Source) -> Result<CacheValue, String> {
    let mut parsed = Database::new();
    parsed.load_font_data(crate::markdown::svg_rasterize::font::BUNDLED_SANS_SERIF_FONT.to_vec());
    let mut face = parsed
        .faces()
        .next()
        .ok_or("bundled font face missing")?
        .clone();
    face.source = source;
    let mut database = Database::new();
    database.push_face_info(face);
    Ok(CacheValue {
        database: Arc::new(database),
        #[cfg(target_os = "macos")]
        proof: None,
    })
}

fn empty_value() -> CacheValue {
    CacheValue {
        database: Arc::new(Database::new()),
        #[cfg(target_os = "macos")]
        proof: None,
    }
}

#[test]
fn retained_mapping_bytes_deduplicates_shared_arc_identity() -> Result<(), String> {
    let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(vec![0_u8; 1024]);
    let first = value_with_source(Source::SharedFile(
        PathBuf::from("/System/Library/Fonts/one.ttc"),
        Arc::clone(&bytes),
    ))?;
    let second = value_with_source(Source::SharedFile(
        PathBuf::from("/System/Library/Fonts/one.ttc"),
        Arc::clone(&bytes),
    ))?;
    let distinct = value_with_source(Source::SharedFile(
        PathBuf::from("/System/Library/Fonts/two.ttc"),
        Arc::new(vec![0_u8; 512]),
    ))?;

    assert_eq!(
        retained_mapping_bytes([&first, &second, &distinct].into_iter()),
        1536
    );
    Ok(())
}

#[test]
fn mapping_budget_accepts_exact_limit_and_rejects_overflow() -> Result<(), String> {
    let shared: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(vec![0_u8; 1024]);
    let first = value_with_source(Source::SharedFile(
        PathBuf::from("/System/Library/Fonts/one.ttc"),
        Arc::clone(&shared),
    ))?;
    let candidate = value_with_source(Source::SharedFile(
        PathBuf::from("/System/Library/Fonts/one.ttc"),
        Arc::clone(&shared),
    ))?;
    assert!(within_mapping_budget(
        [&first, &candidate].into_iter(),
        1024
    ));

    let distinct = value_with_source(Source::SharedFile(
        PathBuf::from("/System/Library/Fonts/two.ttc"),
        Arc::new(vec![0_u8; 1]),
    ))?;
    assert!(!within_mapping_budget(
        [&first, &distinct].into_iter(),
        1024
    ));
    Ok(())
}

#[cfg(target_os = "macos")]
fn owned_shared_source(value: &CacheValue) -> Vec<(PathBuf, SharedBytes)> {
    let Some(proof) = value.proof.as_ref() else {
        return Vec::new();
    };
    value
        .database
        .faces()
        .filter_map(|face| match &face.source {
            Source::SharedFile(path, data) if proof.owns_immutable_source(path, data) => {
                Some((path.clone(), Arc::clone(data)))
            }
            _ => None,
        })
        .collect()
}

#[cfg(target_os = "macos")]
type WeakSharedBytes = std::sync::Weak<dyn AsRef<[u8]> + Send + Sync>;

#[cfg(target_os = "macos")]
fn first_request_weak_sources(
    cache: &mut HtmlFontDatabaseCache,
) -> Result<Vec<(PathBuf, WeakSharedBytes)>, String> {
    let first = cache.get_mode(DatabaseKey {
        request: HtmlFontRequest::from_text("Helvetica", "漢"),
        discovery: super::super::DiscoveryMode::KnownOnly,
    });
    let first_sources = owned_shared_source(&first);
    if first_sources.is_empty() {
        return Err("first request did not own an immutable shared source".into());
    }
    let released: Vec<_> = first_sources
        .iter()
        .map(|(path, data)| (path.clone(), Arc::downgrade(data)))
        .collect();
    drop(first_sources);
    drop(first);
    Ok(released)
}

#[cfg(target_os = "macos")]
fn second_request_reuses_a_source(
    cache: &mut HtmlFontDatabaseCache,
    released: &[(PathBuf, WeakSharedBytes)],
) -> Result<(), String> {
    let second = cache.get_mode(DatabaseKey {
        request: HtmlFontRequest::from_text("Helvetica", "漢😀"),
        discovery: super::super::DiscoveryMode::KnownOnly,
    });
    let second_sources = owned_shared_source(&second);
    for (path, weak) in released {
        let Some((_, second_data)) = second_sources
            .iter()
            .find(|(second_path, _)| second_path == path)
        else {
            continue;
        };
        let Some(first_data) = weak.upgrade() else {
            return Err("mapping released while the second cache entry owned it".into());
        };
        if Arc::ptr_eq(&first_data, second_data) {
            return Ok(());
        }
    }
    Err("second request did not reuse a shared source path".into())
}

#[cfg(target_os = "macos")]
fn evict_seeded_entries(cache: &mut HtmlFontDatabaseCache) {
    for index in 0..super::super::MAX_CACHED_HTML_FONT_DATABASES - 2 {
        let family = format!("__krr_seed_filler_{index}");
        let key = DatabaseKey::eager(HtmlFontRequest::from_text(&family, ""));
        cache.entries.push_back((key, empty_value()));
    }
    for index in 0..2 {
        let family = format!("__krr_seed_eviction_{index}");
        let _ = cache.get_mode(DatabaseKey::eager(HtmlFontRequest::from_text(&family, "")));
    }
}

#[cfg(target_os = "macos")]
#[test]
fn different_requests_reuse_a_verified_mapping_and_lru_eviction_releases_it() -> Result<(), String>
{
    let mut cache = HtmlFontDatabaseCache {
        entries: VecDeque::new(),
    };
    let released = first_request_weak_sources(&mut cache)?;
    second_request_reuses_a_source(&mut cache, &released)?;
    evict_seeded_entries(&mut cache);
    assert!(released.iter().all(|(_, weak)| weak.upgrade().is_none()));
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn zero_budget_builder_keeps_file_sources_unseeded() -> Result<(), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        let request = HtmlFontRequest::from_text("Helvetica", "preload source");
        let (database, _) = sources::build_known_html_font_db_with_shared_budget(&request, 0);
        let mut has_file_source = false;
        for face in database.faces() {
            match &face.source {
                Source::File(_) => has_file_source = true,
                Source::SharedFile(_, _) => {
                    return Err("zero-budget builder created a shared mapping".into());
                }
                _ => {}
            }
        }
        has_file_source
            .then_some(())
            .ok_or_else(|| "zero-budget builder did not keep a File source".into())
    })
}

#[cfg(target_os = "macos")]
fn full_cache_with_mapping() -> Result<HtmlFontDatabaseCache, String> {
    let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(vec![0_u8; 1024]);
    let mut cache = HtmlFontDatabaseCache {
        entries: VecDeque::new(),
    };
    let first_family = "__krr_cached_0";
    let first_key = DatabaseKey::eager(HtmlFontRequest::from_text(first_family, ""));
    cache.entries.push_back((
        first_key,
        value_with_source(Source::SharedFile(
            PathBuf::from("/System/Library/Fonts/one.ttc"),
            bytes,
        ))?,
    ));
    for index in 1..super::super::MAX_CACHED_HTML_FONT_DATABASES {
        let family = format!("__krr_cached_{index}");
        let key = DatabaseKey::eager(HtmlFontRequest::from_text(&family, ""));
        cache.entries.push_back((key, empty_value()));
    }
    Ok(cache)
}

#[cfg(target_os = "macos")]
#[test]
fn full_lru_evicts_oldest_shared_mapping_before_admission() -> Result<(), String> {
    let mut cache = full_cache_with_mapping()?;
    assert_eq!(
        cache.entries.len(),
        super::super::MAX_CACHED_HTML_FONT_DATABASES
    );
    assert_eq!(
        retained_mapping_bytes(cache.entries.iter().map(|(_, value)| value)),
        1024
    );

    let request = HtmlFontRequest::from_text("__krr_lru_eviction_probe__", "");
    let _ = cache.get_mode(DatabaseKey {
        request,
        discovery: super::super::DiscoveryMode::KnownOnly,
    });

    assert_eq!(
        cache.entries.len(),
        super::super::MAX_CACHED_HTML_FONT_DATABASES
    );
    assert_eq!(
        retained_mapping_bytes(cache.entries.iter().map(|(_, value)| value)),
        0
    );
    Ok(())
}
