use std::fs;

use super::super::operations::perform_operation;
use super::super::options::LegacyOperationPlan;
use super::super::{hex_digest, HtmlBrowserSession};

pub(super) fn measure_legacy_post_operation(
    session: &mut HtmlBrowserSession,
    plan: &LegacyOperationPlan,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let (measurement, previous_generation, operation_generation, before_pixels, after_pixels) =
        perform_operation(session, plan)?;
    if before_pixels == after_pixels {
        return Err("post-operation did not change frame pixels".into());
    }
    let expected_state_sha256 = hex_digest(&fs::read(&plan.expected_dom_state_path)?);
    if expected_state_sha256 != plan.expected_dom_state_sha256 {
        return Err("expected DOM state evidence SHA does not match its file".into());
    }
    Ok(serde_json::json!({
        "measurement": measurement, "generation_before": previous_generation,
        "generation_after": operation_generation, "pixel_sha_before": before_pixels,
        "pixel_sha_after": after_pixels, "visual_change_observed": true,
        "semantic_state_evidence": {"lane": "existing-html-interaction-regression",
            "expected_state_file_sha256": expected_state_sha256},
        "operation": plan.operation, "expected_dom_state_sha256": expected_state_sha256,
    }))
}
