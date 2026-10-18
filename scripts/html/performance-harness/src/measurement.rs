use std::{fs, time::Instant};

mod catalog_evidence;
use catalog_evidence::{catalog_condition_report, validate_sample_input};
const RESIZE_WIDTH_REDUCTION_DIVISOR: u32 = 4;

use super::{hex_digest, phase_snapshot, HtmlBrowserSession, HtmlBrowserViewport, SessionFrame};
use super::{
    operations::{measure_full_y, perform_operation, CatalogOperations},
    options::{CatalogOperationPlan, OperationPlan},
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
    validate_sample_input(operation_plan, source_hash, viewport)?;
    let origin = format!("file:///krr-performance/{source_hash}.html");
    let (mut session, cold, warm_phase) = measure_open_phases(html, &origin, viewport)?;
    let refresh_start = Instant::now();
    session.refresh_frame()?;
    let redisplay = phase_snapshot(refresh_start, &session)?;
    let (full_y, scroll_steps, scroll_y, max_scroll) = measure_full_y(&mut session, viewport)?;
    let post_operation =
        measure_post_operation(&mut session, operation_plan, html, &origin, viewport)?;
    let same_session_resize = measure_same_session_resize(&mut session, viewport)?;
    session.close()?;
    let catalog_condition = catalog_condition_report(operation_plan);
    Ok(serde_json::json!({
        "sample": sample,
        "viewport": {"width": viewport.width, "height": viewport.height, "scale": viewport.device_scale_factor},
        "catalog_condition": catalog_condition,
        "phases": {"cold_open": cold, "warm_open": warm_phase, "re_display": redisplay,
            "full_y": {"elapsed_ms": full_y["elapsed_ms"], "max_frame_elapsed_ms": full_y["max_frame_elapsed_ms"], "peak_rss_bytes": full_y["peak_rss_bytes"], "scroll_steps": scroll_steps, "final_scroll_y": scroll_y, "max_scroll": max_scroll, "actual_bottom": true, "frame": full_y["frame"]},
            "post_operation": post_operation, "same_session_resize": same_session_resize},
    }))
}

fn measure_same_session_resize(
    session: &mut HtmlBrowserSession,
    viewport: HtmlBrowserViewport,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let before = session
        .latest_frame()
        .ok_or("resize requires a prepared frame")?;
    let generation_before = before.generation;
    let resized_viewport = narrower_resize_viewport(viewport)?;
    let resize_start = Instant::now();
    session.resize(resized_viewport)?;
    let measurement = phase_snapshot(resize_start, session)?;
    let generation_after = validate_resized_frame(session, resized_viewport, generation_before)?;
    Ok(serde_json::json!({
        "measurement": measurement,
        "resize_evidence": {
            "from_viewport": {"width": viewport.width, "height": viewport.height, "scale": viewport.device_scale_factor},
            "to_viewport": {"width": resized_viewport.width, "height": resized_viewport.height, "scale": resized_viewport.device_scale_factor},
            "generation_before": generation_before,
            "generation_after": generation_after,
            "real_raster_frame": true,
        },
    }))
}

fn narrower_resize_viewport(
    viewport: HtmlBrowserViewport,
) -> Result<HtmlBrowserViewport, Box<dyn std::error::Error>> {
    if viewport.width > 1 {
        let reduction = (viewport.width / RESIZE_WIDTH_REDUCTION_DIVISOR).max(1);
        Ok(HtmlBrowserViewport::new(
            viewport.width - reduction,
            viewport.height,
            viewport.device_scale_factor,
        )?)
    } else {
        Ok(HtmlBrowserViewport::new(
            2,
            viewport.height,
            viewport.device_scale_factor,
        )?)
    }
}

fn validate_resized_frame(
    session: &HtmlBrowserSession,
    resized_viewport: HtmlBrowserViewport,
    generation_before: u64,
) -> Result<u64, Box<dyn std::error::Error>> {
    let frame = session
        .latest_frame()
        .ok_or("resize did not produce a frame")?;
    if frame.generation <= generation_before {
        return Err("resize did not advance frame generation".into());
    }
    if frame.viewport != resized_viewport
        || frame.pixels.is_empty()
        || !frame.scroll_y.is_finite()
        || !frame.content_height.is_finite()
    {
        return Err("resize produced an invalid raster frame".into());
    }
    Ok(frame.generation)
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
    html: &[u8],
    origin: &str,
    viewport: HtmlBrowserViewport,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    match operation_plan {
        OperationPlan::Catalog(plan) => {
            measure_catalog_post_operation(session, plan, html, origin, viewport)
        }
        OperationPlan::Legacy(plan) => measure_legacy_post_operation(session, plan),
    }
}

fn measure_catalog_post_operation(
    session: &mut HtmlBrowserSession,
    plan: &CatalogOperationPlan,
    html: &[u8],
    origin: &str,
    viewport: HtmlBrowserViewport,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if !CatalogOperations::is_static(plan) {
        return CatalogOperations::perform(session, plan);
    }
    let operation_start = Instant::now();
    session.close()?;
    let source = HtmlBrowserSource::new(String::from_utf8(html.to_vec())?, origin)?;
    *session = HtmlRuntime.open(source, viewport)?;
    Ok(serde_json::json!({
        "measurement": phase_snapshot(operation_start, session)?,
        "catalog_operation": {
            "condition_id": plan.condition_id,
            "case_id": plan.case_id,
            "state": plan.state,
            "operation_kind": plan.operation_kind,
            "operation_evidence_sha256": plan.operation_evidence_sha256,
            "delivery": "measured-close-and-new-session-open",
            "frame_after": session.frame_state()?,
        }
    }))
}

fn measure_legacy_post_operation(
    session: &mut HtmlBrowserSession,
    plan: &super::options::LegacyOperationPlan,
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
