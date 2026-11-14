use super::super::ResourceFile;
use super::super::identity::FontResolutionCache;
use super::{CaptureRequest, CaptureResult, SelectedFont, resolve_selected_font};
use resvg::usvg::fontdb::Database;

pub(super) fn probe_capture(
    system_database: &Database,
    capture: CaptureRequest,
    resolution_cache: &mut FontResolutionCache<'_>,
) -> CaptureResult {
    let (resource_database, resource_error) = capture_resource_database(&capture.resource_files);
    let mut result = initial_capture_result(
        capture.condition_id,
        capture.selected_fonts.is_empty(),
        resource_error.as_deref(),
    );
    resolve_capture_fonts(
        system_database,
        &resource_database,
        &capture.resource_files,
        resource_error.as_deref(),
        capture.selected_fonts,
        resolution_cache,
        &mut result,
    );
    result
}

fn capture_resource_database(resources: &[ResourceFile]) -> (Database, Option<String>) {
    match super::load_resource_database(resources) {
        Ok(database) => (database, None),
        Err(error) => (Database::new(), Some(error)),
    }
}

fn initial_capture_result(
    condition_id: String,
    has_no_selected_fonts: bool,
    resource_error: Option<&str>,
) -> CaptureResult {
    let mut result = CaptureResult {
        condition_id,
        status: "resolved".to_string(),
        font_provenance: Vec::new(),
        unknown_reasons: Vec::new(),
    };
    if has_no_selected_fonts {
        result.status = "unknown".to_string();
        result
            .unknown_reasons
            .push("Chrome boundary inspection returned no selected fonts".to_string());
    }
    if let Some(error) = resource_error {
        result.status = "unknown".to_string();
        result.unknown_reasons.push(format!(
            "captured font resource inventory is invalid: {error}"
        ));
    }
    result
}

fn resolve_capture_fonts(
    system_database: &Database,
    resource_database: &Database,
    resources: &[ResourceFile],
    resource_error: Option<&str>,
    selected_fonts: Vec<SelectedFont>,
    resolution_cache: &mut FontResolutionCache<'_>,
    result: &mut CaptureResult,
) {
    for selected in selected_fonts {
        resolve_selected_font(
            system_database,
            resource_database,
            resources,
            resource_error,
            selected,
            resolution_cache,
            result,
        );
    }
}
