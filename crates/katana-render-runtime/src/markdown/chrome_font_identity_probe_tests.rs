mod cli;
mod cli_cases;
mod identity;
mod identity_cases;
#[cfg(target_os = "macos")]
mod identity_coretext_cases;
mod probe;
mod test_support;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const REQUEST_SCHEMA: &str = "krr-chrome-font-identity-request-v1";
const RESPONSE_SCHEMA: &str = "krr-chrome-font-identity-response-v1";
const INVENTORY_METHOD: &str = "independent fontdb metadata parse; exact PostScript-name match; physical file SHA and face index verified; macOS URL-bound CoreText descriptors provide named-face metadata, not Chrome-rendered variation-instance evidence";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ProbeRequest {
    schema: String,
    captures: Vec<CaptureRequest>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CaptureRequest {
    condition_id: String,
    selected_fonts: Vec<SelectedFont>,
    resource_files: Vec<ResourceFile>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SelectedFont {
    family_name: String,
    post_script_name: String,
    glyph_count: u64,
    is_custom_font: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ResourceFile {
    url: String,
    body_path: PathBuf,
    bytes_sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ProbeResponse {
    schema: String,
    request_sha256: String,
    inventory_method: String,
    inventory_limit: String,
    captures: Vec<CaptureResult>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CaptureResult {
    condition_id: String,
    status: String,
    font_provenance: Vec<FontProvenance>,
    unknown_reasons: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FontProvenance {
    scope: String,
    family: String,
    source: String,
    file_path: String,
    file_sha256: String,
    face_index: u32,
    selected_node_id: Option<u64>,
    selected: bool,
    file_bytes_verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    core_text_descriptor: Option<CoreTextDescriptorEvidence>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CoreTextDescriptorEvidence {
    url_path: String,
    post_script_name: String,
    variation_axes: Vec<CoreTextVariationAxis>,
    variation_coordinates: Vec<CoreTextVariationCoordinate>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CoreTextVariationAxis {
    identifier: u32,
    minimum_value: f64,
    maximum_value: f64,
    default_value: f64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CoreTextVariationCoordinate {
    identifier: u32,
    value: f64,
}

#[test]
#[ignore = "manual bridge: read CDP selected-font request JSON from KRR_CHROME_FONT_IDENTITY_REQUEST"]
fn chrome_font_identity_probe_cli() -> Result<(), String> {
    cli::run()
}

#[test]
fn observed_system_face_resolves_to_its_physical_file_and_index() -> Result<(), String> {
    identity_cases::observed_system_face_resolves_to_its_physical_file_and_index()
}

#[cfg(target_os = "macos")]
#[test]
fn coretext_url_descriptor_proves_sfns_bold_variable_instance() -> Result<(), String> {
    identity_coretext_cases::coretext_url_descriptor_proves_sfns_bold_variable_instance()
}

#[cfg(target_os = "macos")]
#[test]
fn coretext_fallback_rejects_collections_and_nonzero_face_indices() {
    identity_coretext_cases::coretext_fallback_rejects_collections_and_nonzero_face_indices()
}

#[test]
fn missing_and_ambiguous_physical_faces_remain_unknown() -> Result<(), String> {
    identity_cases::missing_and_ambiguous_physical_faces_remain_unknown()
}

#[test]
fn changed_custom_resource_bytes_are_rejected() -> Result<(), String> {
    identity_cases::changed_custom_resource_bytes_are_rejected()
}

#[test]
fn ignored_probe_cli_writes_strict_response_file_and_refuses_reuse() -> Result<(), String> {
    cli_cases::ignored_probe_cli_writes_strict_response_file_and_refuses_reuse()
}
