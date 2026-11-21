use super::{HtmlFontRequest, MAX_CACHED_HTML_FONT_DATABASES, discovery_cache};

pub(super) fn evict_cached_mappings() -> Result<(), String> {
    for index in 0..MAX_CACHED_HTML_FONT_DATABASES {
        let family = format!("__krr_lru_eviction_probe_{index}");
        let request = HtmlFontRequest::from_text(&family, "");
        discovery_cache::known_for(&request).ok_or("LRU probe database rejected")?;
    }
    Ok(())
}
