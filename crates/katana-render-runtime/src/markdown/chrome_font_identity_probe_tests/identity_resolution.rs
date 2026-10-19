use self::matching::{IdentityMap, matching_identities, unique_identity, verify_physical_face};
use super::super::{ResourceFile, SelectedFont};
use super::ResolvedFont;
use resvg::usvg::fontdb::Database;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

#[path = "identity_matching.rs"]
mod matching;

pub(in crate::markdown::chrome_font_identity_probe_tests) struct FontResolutionCache<'database> {
    system_database: &'database Database,
    direct_system: BTreeMap<(String, String), Result<ResolvedFont, String>>,
    #[cfg(target_os = "macos")]
    coretext: BTreeMap<(String, String), Result<ResolvedFont, String>>,
}

impl<'database> FontResolutionCache<'database> {
    pub(in crate::markdown::chrome_font_identity_probe_tests) fn for_system_database(
        system_database: &'database Database,
    ) -> Self {
        Self {
            system_database,
            direct_system: BTreeMap::new(),
            #[cfg(target_os = "macos")]
            coretext: BTreeMap::new(),
        }
    }

    pub(in crate::markdown::chrome_font_identity_probe_tests) fn verify_physical_inputs(
        &self,
    ) -> Result<(), String> {
        let resolutions = self.direct_system.values();
        #[cfg(target_os = "macos")]
        let resolutions = resolutions.chain(self.coretext.values());
        let mut verified = BTreeSet::new();
        for resolved in resolutions.filter_map(|resolution| resolution.as_ref().ok()) {
            if verified.insert(resolved.identity.clone()) {
                verify_physical_face(&resolved.identity)?;
            }
        }
        Ok(())
    }
}

pub(in crate::markdown::chrome_font_identity_probe_tests) fn resolve_font_with_cache(
    system_database: &Database,
    resource_database: &Database,
    resources: &[ResourceFile],
    selected: &SelectedFont,
    cache: &mut FontResolutionCache<'_>,
) -> Result<ResolvedFont, String> {
    validate_selected_font(selected)?;
    if !selected.is_custom_font {
        return resolve_cached_system_font(
            system_database,
            resource_database,
            resources,
            selected,
            cache,
        );
    }
    resolve_font_uncached(
        system_database,
        resource_database,
        resources,
        selected,
        cache,
    )
}

fn resolve_cached_system_font(
    system_database: &Database,
    resource_database: &Database,
    resources: &[ResourceFile],
    selected: &SelectedFont,
    cache: &mut FontResolutionCache<'_>,
) -> Result<ResolvedFont, String> {
    if !std::ptr::eq(cache.system_database, system_database) {
        return Err("system font cache Database binding differs".to_string());
    }
    let key = (
        selected.family_name.clone(),
        selected.post_script_name.clone(),
    );
    if let Some(cached) = cache.direct_system.get(&key) {
        return cached.clone();
    }
    let result = resolve_font_uncached(
        system_database,
        resource_database,
        resources,
        selected,
        cache,
    );
    cache.direct_system.insert(key, result.clone());
    result
}

fn resolve_font_uncached(
    system_database: &Database,
    resource_database: &Database,
    resources: &[ResourceFile],
    selected: &SelectedFont,
    cache: &mut FontResolutionCache<'_>,
) -> Result<ResolvedFont, String> {
    let database = if selected.is_custom_font {
        resource_database
    } else {
        system_database
    };
    let allowed_paths = resource_paths(resources);
    let identities = matching_identities(database, selected, &allowed_paths)?;
    if let Some(resolved) = resolved_font(identities, selected)? {
        return Ok(resolved);
    }
    resolve_missing_font(database, selected, cache)
}

fn resolved_font(
    identities: IdentityMap,
    selected: &SelectedFont,
) -> Result<Option<ResolvedFont>, String> {
    if identities.is_empty() {
        return Ok(None);
    }
    let method = if selected.is_custom_font {
        "captured Chrome resource response body"
    } else {
        "observed fontdb system-font inventory"
    };
    Ok(Some(ResolvedFont {
        identity: unique_identity(identities)?,
        method,
        core_text_descriptor: None,
    }))
}

fn resolve_missing_font(
    database: &Database,
    selected: &SelectedFont,
    cache: &mut FontResolutionCache<'_>,
) -> Result<ResolvedFont, String> {
    #[cfg(target_os = "macos")]
    return if selected.is_custom_font {
        Err("no exact physical face matched the CDP PostScript name".to_string())
    } else {
        resolve_system_font_with_coretext(database, selected, cache)
    };
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (database, selected, cache);
        Err("no exact physical face matched the CDP PostScript name".to_string())
    }
}

#[cfg(target_os = "macos")]
fn resolve_system_font_with_coretext(
    database: &Database,
    selected: &SelectedFont,
    cache: &mut FontResolutionCache<'_>,
) -> Result<ResolvedFont, String> {
    let key = (
        selected.family_name.clone(),
        selected.post_script_name.clone(),
    );
    cache
        .coretext
        .entry(key)
        .or_insert_with(|| super::coretext::resolve_font(database, selected))
        .clone()
}

fn validate_selected_font(selected: &SelectedFont) -> Result<(), String> {
    if selected.family_name.trim().is_empty()
        || selected.post_script_name.trim().is_empty()
        || selected.glyph_count == 0
    {
        return Err("CDP selected-font identity is incomplete".to_string());
    }
    Ok(())
}

fn resource_paths(resources: &[ResourceFile]) -> BTreeSet<PathBuf> {
    resources
        .iter()
        .filter_map(|resource| fs::canonicalize(&resource.body_path).ok())
        .collect()
}
