use super::{
    CaptureRequest, CaptureResult, FontProvenance, INVENTORY_METHOD, ProbeRequest, ProbeResponse,
    REQUEST_SCHEMA, RESPONSE_SCHEMA, SelectedFont,
};
use crate::markdown::chrome_font_identity_probe_tests::identity::{
    FontResolutionCache, load_resource_database, resolve_font_with_cache, sha256,
};
use resvg::usvg::fontdb::Database;
use std::collections::BTreeSet;

#[path = "probe_capture.rs"]
mod capture;
use capture::probe_capture;

pub(super) fn probe(request_bytes: &[u8]) -> Result<ProbeResponse, String> {
    let request: ProbeRequest =
        serde_json::from_slice(request_bytes).map_err(|error| format!("parse request: {error}"))?;
    validate_request(&request)?;
    let mut system_database = Database::new();
    system_database.load_system_fonts();
    let mut resolution_cache = FontResolutionCache::for_system_database(&system_database);
    let captures = request
        .captures
        .into_iter()
        .map(|capture| probe_capture(&system_database, capture, &mut resolution_cache))
        .collect();
    resolution_cache.verify_physical_inputs()?;
    Ok(ProbeResponse {
        schema: RESPONSE_SCHEMA.to_string(),
        request_sha256: sha256(request_bytes),
        inventory_method: INVENTORY_METHOD.to_string(),
        inventory_limit: "uniqueness is within fontdb's observed system inventory and explicitly captured resource files; unobserved font stores are not asserted absent. CoreText fallback requires a single-font file at fontdb face index 0 because its URL descriptor API exposes no face index; collection files remain unknown. Descriptor variation coordinates are named-face metadata, not proof of the exact variable instance rendered by Chrome.".to_string(),
        captures,
    })
}

fn validate_request(request: &ProbeRequest) -> Result<(), String> {
    if request.schema != REQUEST_SCHEMA || request.captures.is_empty() {
        return Err("unsupported request schema or empty capture list".to_string());
    }
    let mut condition_ids = BTreeSet::new();
    for capture in &request.captures {
        validate_capture_identity(capture, &mut condition_ids)?;
    }
    Ok(())
}

fn validate_capture_identity(
    capture: &CaptureRequest,
    condition_ids: &mut BTreeSet<String>,
) -> Result<(), String> {
    if capture.condition_id.trim().is_empty() || !condition_ids.insert(capture.condition_id.clone())
    {
        return Err("capture condition IDs must be non-empty and unique".to_string());
    }
    validate_unique_selected_fonts(&capture.selected_fonts)
}

fn validate_unique_selected_fonts(fonts: &[SelectedFont]) -> Result<(), String> {
    let mut font_keys = BTreeSet::new();
    for font in fonts {
        let key = (
            font.family_name.clone(),
            font.post_script_name.clone(),
            font.is_custom_font,
        );
        if !font_keys.insert(key) {
            return Err("duplicate CDP selected-font entry".to_string());
        }
    }
    Ok(())
}

fn resolve_selected_font(
    system_database: &Database,
    resource_database: &Database,
    resources: &[super::ResourceFile],
    resource_error: Option<&str>,
    selected: SelectedFont,
    resolution_cache: &mut FontResolutionCache<'_>,
    result: &mut CaptureResult,
) {
    let resolution = resolve_selected_identity(
        system_database,
        resource_database,
        resources,
        resource_error,
        &selected,
        resolution_cache,
    );
    record_selected_resolution(selected, resolution, result);
}

fn resolve_selected_identity(
    system_database: &Database,
    resource_database: &Database,
    resources: &[super::ResourceFile],
    resource_error: Option<&str>,
    selected: &SelectedFont,
    resolution_cache: &mut FontResolutionCache<'_>,
) -> Result<super::identity::ResolvedFont, String> {
    if let Some(error) = custom_resource_error(selected.is_custom_font, resource_error) {
        return Err(error);
    }
    resolve_font_with_cache(
        system_database,
        resource_database,
        resources,
        selected,
        resolution_cache,
    )
}

fn record_selected_resolution(
    selected: SelectedFont,
    resolution: Result<super::identity::ResolvedFont, String>,
    result: &mut CaptureResult,
) {
    match resolution {
        Ok(resolved) => result
            .font_provenance
            .push(font_provenance(selected, resolved)),
        Err(reason) => {
            result.status = "unknown".to_string();
            result
                .unknown_reasons
                .push(unknown_reason(&selected, &reason));
        }
    }
}

fn font_provenance(
    selected: SelectedFont,
    resolved: super::identity::ResolvedFont,
) -> FontProvenance {
    let identity = resolved.identity;
    FontProvenance {
        scope: "capture".to_string(),
        family: selected.family_name,
        source: format!(
            "Chrome CDP postScriptName={} isCustomFont={}; {}; exact PostScript-name match; physical file SHA and face index verified",
            selected.post_script_name, selected.is_custom_font, resolved.method
        ),
        file_path: identity.path.display().to_string(),
        file_sha256: identity.file_sha256,
        face_index: identity.face_index,
        selected_node_id: None,
        selected: true,
        file_bytes_verified: true,
        core_text_descriptor: resolved.core_text_descriptor,
    }
}

fn unknown_reason(selected: &SelectedFont, reason: &str) -> String {
    format!(
        "{} ({}, isCustomFont={}): {reason}",
        selected.family_name, selected.post_script_name, selected.is_custom_font
    )
}

fn custom_resource_error(is_custom: bool, resource_error: Option<&str>) -> Option<String> {
    if is_custom {
        resource_error.map(|error| format!("captured font resource inventory is invalid: {error}"))
    } else {
        None
    }
}
