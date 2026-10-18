use super::super::{ResourceFile, SelectedFont};
#[cfg(target_os = "macos")]
use super::physical_file;
use super::{PhysicalFace, ResolvedFont, face_identity};
use resvg::usvg::fontdb::{Database, FaceInfo};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

#[derive(Default)]
pub(in crate::markdown::chrome_font_identity_probe_tests) struct FontResolutionCache {
    #[cfg(target_os = "macos")]
    coretext: BTreeMap<(String, String), Result<ResolvedFont, String>>,
}

impl FontResolutionCache {
    pub(in crate::markdown::chrome_font_identity_probe_tests) fn verify_physical_inputs(
        &self,
    ) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        for resolution in self.coretext.values() {
            let Ok(resolved) = resolution else {
                continue;
            };
            verify_physical_face(&resolved.identity)?;
        }
        Ok(())
    }
}

type IdentityMap = BTreeMap<(PathBuf, u32), PhysicalFace>;

pub(in crate::markdown::chrome_font_identity_probe_tests) fn resolve_font(
    system_database: &Database,
    resource_database: &Database,
    resources: &[ResourceFile],
    selected: &SelectedFont,
) -> Result<ResolvedFont, String> {
    let mut cache = FontResolutionCache::default();
    let resolved = resolve_font_with_cache(
        system_database,
        resource_database,
        resources,
        selected,
        &mut cache,
    )?;
    cache.verify_physical_inputs()?;
    Ok(resolved)
}

pub(in crate::markdown::chrome_font_identity_probe_tests) fn resolve_font_with_cache(
    system_database: &Database,
    resource_database: &Database,
    resources: &[ResourceFile],
    selected: &SelectedFont,
    cache: &mut FontResolutionCache,
) -> Result<ResolvedFont, String> {
    validate_selected_font(selected)?;
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
    cache: &mut FontResolutionCache,
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
    cache: &mut FontResolutionCache,
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

fn matching_identities(
    database: &Database,
    selected: &SelectedFont,
    allowed_paths: &BTreeSet<PathBuf>,
) -> Result<IdentityMap, String> {
    let mut identities = BTreeMap::new();
    for face in database
        .faces()
        .filter(|face| face.post_script_name == selected.post_script_name)
    {
        insert_matching_identity(&mut identities, database, face, selected, allowed_paths)?;
    }
    Ok(identities)
}

fn insert_matching_identity(
    identities: &mut IdentityMap,
    database: &Database,
    face: &FaceInfo,
    selected: &SelectedFont,
    allowed_paths: &BTreeSet<PathBuf>,
) -> Result<(), String> {
    let identity = face_identity(database, face)?;
    if selected.is_custom_font && !allowed_paths.contains(&identity.path) {
        return Ok(());
    }
    identities.insert((identity.path.clone(), identity.face_index), identity);
    Ok(())
}

fn unique_identity(identities: IdentityMap) -> Result<PhysicalFace, String> {
    match identities.len() {
        0 => Err("no exact physical face matched the CDP PostScript name".to_string()),
        1 => identities
            .into_values()
            .next()
            .ok_or_else(|| "resolved font face disappeared".to_string()),
        _ => Err("multiple physical file/face-index matches are ambiguous".to_string()),
    }
}

#[cfg(target_os = "macos")]
fn verify_physical_face(face: &PhysicalFace) -> Result<(), String> {
    let (path, digest, _) = physical_file(&face.path)?;
    if path != face.path || digest != face.file_sha256 {
        return Err("physical font changed before the probe response was written".to_string());
    }
    Ok(())
}
