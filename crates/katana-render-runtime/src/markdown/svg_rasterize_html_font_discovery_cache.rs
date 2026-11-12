#[cfg(test)]
use super::sources;
use super::{
    CacheValue, DatabaseKey, HtmlFontDatabaseCache, HtmlFontRequest,
    MAX_CACHED_HTML_FONT_DATABASES, MeasuredDatabase, html_font_db_for_text,
};
#[cfg(target_os = "macos")]
use super::{DiscoveryMode, discovery::KnownCacheValue, html_font_cache};
#[path = "svg_rasterize_html_font_discovery_cache_build.rs"]
mod cache_build;
use cache_build::build_miss;
#[cfg(test)]
use cache_build::{retained_mapping_bytes, within_mapping_budget};
use resvg::usvg;
use std::sync::Arc;

impl HtmlFontDatabaseCache {
    pub(super) fn get_mode(&mut self, key: DatabaseKey) -> CacheValue {
        if let Some(index) = self.entries.iter().position(|(cached, _)| cached == &key)
            && let Some(entry) = self.entries.remove(index)
        {
            let result = copy_value(&entry.1);
            self.entries.push_back(entry);
            return result;
        }
        self.evict_oldest_if_full();
        let value = build_miss(&key, &self.entries);
        let result = copy_value(&value);
        self.entries.push_back((key, value));
        result
    }
}

impl HtmlFontDatabaseCache {
    fn evict_oldest_if_full(&mut self) {
        if self.entries.len() >= MAX_CACHED_HTML_FONT_DATABASES {
            self.entries.pop_front();
        }
    }
}

fn copy_value(value: &CacheValue) -> CacheValue {
    CacheValue {
        database: Arc::clone(&value.database),
        #[cfg(target_os = "macos")]
        proof: value.proof.clone(),
    }
}

#[cfg(target_os = "macos")]
pub(super) fn known_for(request: &HtmlFontRequest) -> Option<KnownCacheValue> {
    let mut cache = html_font_cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let value = cache.get_mode(DatabaseKey {
        request: request.clone(),
        discovery: DiscoveryMode::KnownOnly,
    });
    let proof = value.proof?;
    proof.current(&value.database).then_some(KnownCacheValue {
        database: value.database,
        proof,
    })
}

#[cfg(target_os = "macos")]
fn cached_known_value(request: &HtmlFontRequest) -> CacheValue {
    let mut cache = html_font_cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    cache.get_mode(DatabaseKey {
        request: request.clone(),
        discovery: DiscoveryMode::KnownOnly,
    })
}

#[cfg(target_os = "macos")]
pub(super) fn measured_known(
    request: &HtmlFontRequest,
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) -> Option<MeasuredDatabase> {
    if !text.is_ascii() || !super::super::super::text_shaping::known_memo_usable() {
        return None;
    }
    /* WHY: admitの入場・出口で全sourceを確認するため、直前の同じproof確認を重ねない。 */
    let value = cached_known_value(request);
    super::admission::admit(
        &value.database,
        value.proof.as_ref()?,
        family,
        text,
        weight,
        italic,
    )?;
    Some(MeasuredDatabase {
        key: DatabaseKey {
            request: request.clone(),
            discovery: DiscoveryMode::KnownOnly,
        },
        database: value.database,
    })
}

pub(in crate::markdown::svg_rasterize) fn html_font_db_for_measured_text(
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) -> Arc<usvg::fontdb::Database> {
    measured_database(family, text, weight, italic).database
}

pub(in crate::markdown::svg_rasterize) fn measured_database(
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) -> MeasuredDatabase {
    let request = HtmlFontRequest::from_text(family, text);
    #[cfg(target_os = "macos")]
    if let Some(known) = measured_known(&request, family, text, weight, italic) {
        #[cfg(test)]
        super::decision_observer::record(&known, family, text, weight, italic);
        return known;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (weight, italic);
    let key = DatabaseKey::eager(request);
    let database = html_font_db_for_text(family, text);
    let selected = MeasuredDatabase { key, database };
    #[cfg(all(test, target_os = "macos"))]
    super::decision_observer::record(&selected, family, text, weight, italic);
    selected
}

#[cfg(test)]
#[path = "svg_rasterize_html_font_discovery_cache_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_font_discovery_cache_lock_tests.rs"]
mod lock_tests;
