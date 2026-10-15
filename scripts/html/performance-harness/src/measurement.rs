use std::{fs, time::Instant};

use super::{hex_digest, phase_snapshot, HtmlBrowserSession, HtmlBrowserViewport};
use super::{
    operations::{measure_full_y, perform_operation},
    options::OperationPlan,
};
#[cfg(feature = "baseline")]
use baseline::{HtmlBrowserSource, HtmlRuntime};
#[cfg(feature = "candidate")]
use candidate::{HtmlBrowserSource, HtmlRuntime};

pub(super) fn measure_one(
    html: &[u8],
    source_hash: &str,
    viewport: HtmlBrowserViewport,
    sample: usize,
    operation_plan: &OperationPlan,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if operation_plan.fixture_sha256 != source_hash {
        return Err("operation plan fixture SHA does not match input".into());
    }
    let origin = format!("file:///krr-performance/{source_hash}.html");
    let (mut session, cold, warm_phase) = measure_open_phases(html, &origin, viewport)?;
    let refresh_start = Instant::now();
    session.refresh_frame()?;
    let redisplay = phase_snapshot(refresh_start, &session)?;
    let (full_y, scroll_steps, scroll_y, max_scroll) = measure_full_y(&mut session, viewport)?;
    let post_operation = measure_post_operation(&mut session, operation_plan)?;
    session.close()?;
    Ok(serde_json::json!({
        "sample": sample,
        "viewport": {"width": viewport.width, "height": viewport.height, "scale": viewport.device_scale_factor},
        "phases": {"cold_open": cold, "warm_open": warm_phase, "re_display": redisplay,
            "full_y": {"elapsed_ms": full_y["elapsed_ms"], "peak_rss_bytes": full_y["peak_rss_bytes"], "scroll_steps": scroll_steps, "final_scroll_y": scroll_y, "max_scroll": max_scroll, "actual_bottom": true, "frame": full_y["frame"]},
            "post_operation": post_operation},
    }))
}

type OpenMeasurements = (HtmlBrowserSession, serde_json::Value, serde_json::Value);

fn measure_open_phases(
    html: &[u8],
    origin: &str,
    viewport: HtmlBrowserViewport,
) -> Result<OpenMeasurements, Box<dyn std::error::Error>> {
    let cold_start = Instant::now();
    let source = HtmlBrowserSource::new(String::from_utf8(html.to_vec())?, origin)?;
    let session = HtmlRuntime.open(source, viewport)?;
    let cold = phase_snapshot(cold_start, &session)?;
    let warm_start = Instant::now();
    let warm_source = HtmlBrowserSource::new(String::from_utf8(html.to_vec())?, origin)?;
    let mut warm = HtmlRuntime.open(warm_source, viewport)?;
    let warm_phase = phase_snapshot(warm_start, &warm)?;
    warm.close()?;
    Ok((session, cold, warm_phase))
}

fn measure_post_operation(
    session: &mut HtmlBrowserSession,
    operation_plan: &OperationPlan,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let (measurement, previous_generation, operation_generation, before_pixels, after_pixels) =
        perform_operation(session, operation_plan)?;
    if before_pixels == after_pixels {
        return Err("post-operation did not change frame pixels".into());
    }
    let expected_state_sha256 = hex_digest(&fs::read(&operation_plan.expected_dom_state_path)?);
    if expected_state_sha256 != operation_plan.expected_dom_state_sha256 {
        return Err("expected DOM state evidence SHA does not match its file".into());
    }
    Ok(serde_json::json!({
        "measurement": measurement, "generation_before": previous_generation,
        "generation_after": operation_generation, "pixel_sha_before": before_pixels,
        "pixel_sha_after": after_pixels, "visual_change_observed": true,
        "semantic_state_evidence": {"lane": "existing-html-interaction-regression",
            "expected_state_file_sha256": expected_state_sha256},
        "operation": operation_plan.operation, "expected_dom_state_sha256": expected_state_sha256,
    }))
}
