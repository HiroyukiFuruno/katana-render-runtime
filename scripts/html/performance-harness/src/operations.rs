use std::time::Instant;

use super::options::LegacyOperationPlan;
use super::{hex_digest, phase_snapshot, HtmlBrowserSession, HtmlBrowserViewport, SessionFrame};

mod catalog;
#[cfg(feature = "baseline")]
use baseline::HtmlBrowserInput;
#[cfg(feature = "candidate")]
use candidate::HtmlBrowserInput;
pub(super) use catalog::CatalogOperations;

const SCROLL_STEP: f32 = 600.0;
const POINTER_EVENT_SEQUENCE_LENGTH: usize = 3;
const MILLISECONDS_PER_SECOND: f64 = 1000.0;

pub(super) type OperationMeasurement = (serde_json::Value, u64, u64, String, String);

pub(super) fn perform_operation(
    session: &mut HtmlBrowserSession,
    operation_plan: &LegacyOperationPlan,
) -> Result<OperationMeasurement, Box<dyn std::error::Error>> {
    let before_frame = session
        .latest_frame()
        .ok_or("post-operation requires a prepared frame")?;
    let previous_generation = before_frame.generation;
    let before_pixels = hex_digest(&before_frame.pixels);
    let post_operation = measure_pointer_action(session, operation_plan)?;
    let after_frame = session
        .latest_frame()
        .ok_or("post-operation did not produce a frame")?;
    let operation_generation = after_frame.generation;
    if operation_generation <= previous_generation {
        return Err("post-operation did not advance frame generation".into());
    }
    let after_pixels = hex_digest(&after_frame.pixels);
    Ok((
        post_operation,
        previous_generation,
        operation_generation,
        before_pixels,
        after_pixels,
    ))
}

fn measure_pointer_action(
    session: &mut HtmlBrowserSession,
    operation_plan: &LegacyOperationPlan,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let operation_start = Instant::now();
    for input in pointer_click_inputs(operation_plan.x, operation_plan.y) {
        session.dispatch_input(input)?;
    }
    phase_snapshot(operation_start, session)
}

pub(super) type FullYMeasurement = (serde_json::Value, usize, f32, f32);

pub(super) fn measure_full_y(
    session: &mut HtmlBrowserSession,
    viewport: HtmlBrowserViewport,
) -> Result<FullYMeasurement, Box<dyn std::error::Error>> {
    let full_y_start = Instant::now();
    let mut scroll_steps = 0usize;
    let initial_frame = session
        .latest_frame()
        .ok_or("full-Y requires an initial frame")?;
    let mut scroll_y = initial_frame.scroll_y;
    let max_scroll = (initial_frame.content_height.round() - viewport.logical_height()).max(0.0);
    if max_scroll == 0.0 {
        if scroll_y != 0.0 {
            return Err("short-document full-Y capture must begin at the top".into());
        }
        let mut full_y = phase_snapshot(full_y_start, session)?;
        set_max_frame_elapsed_ms(&mut full_y, 0.0)?;
        return Ok((full_y, scroll_steps, scroll_y, max_scroll));
    }
    let bottom_frame_elapsed_ms =
        scroll_to_bottom(session, &mut scroll_y, max_scroll, &mut scroll_steps)?;
    let actual_bottom = scroll_y == max_scroll;
    if !actual_bottom {
        return Err("full-Y did not reach the actual bottom clamp".into());
    }
    let full_y =
        finish_full_y_measurement(session, full_y_start, max_scroll, bottom_frame_elapsed_ms)?;
    Ok((full_y, scroll_steps, scroll_y, max_scroll))
}

fn finish_full_y_measurement(
    session: &mut HtmlBrowserSession,
    full_y_start: Instant,
    max_scroll: f32,
    bottom_frame_elapsed_ms: f64,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let bottom_frame = session.frame_state()?;
    let top_frame_elapsed_ms = prepare_top_scroll(session, max_scroll)?;
    let mut full_y = phase_snapshot(full_y_start, session)?;
    full_y["frame"] = bottom_frame;
    set_max_frame_elapsed_ms(
        &mut full_y,
        bottom_frame_elapsed_ms.max(top_frame_elapsed_ms),
    )?;
    Ok(full_y)
}

fn set_max_frame_elapsed_ms(
    measurement: &mut serde_json::Value,
    max_frame_elapsed_ms: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let measurement = measurement
        .as_object_mut()
        .ok_or("full-Y measurement must be a JSON object")?;
    measurement.insert(
        "max_frame_elapsed_ms".to_string(),
        serde_json::json!(max_frame_elapsed_ms),
    );
    Ok(())
}

fn scroll_to_bottom(
    session: &mut HtmlBrowserSession,
    scroll_y: &mut f32,
    max_scroll: f32,
    scroll_steps: &mut usize,
) -> Result<f64, Box<dyn std::error::Error>> {
    let mut max_frame_elapsed_ms = 0.0_f64;
    while *scroll_y < max_scroll {
        let frame_start = Instant::now();
        session.dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: SCROLL_STEP,
        })?;
        *scroll_steps += 1;
        let next = session
            .latest_frame()
            .ok_or("full-Y scroll did not produce a frame")?
            .scroll_y;
        if next <= *scroll_y {
            return Err("full-Y scroll made no progress before bottom".into());
        }
        *scroll_y = next;
        max_frame_elapsed_ms =
            max_frame_elapsed_ms.max(frame_start.elapsed().as_secs_f64() * MILLISECONDS_PER_SECOND);
    }
    Ok(max_frame_elapsed_ms)
}

fn prepare_top_scroll(
    session: &mut HtmlBrowserSession,
    max_scroll: f32,
) -> Result<f64, Box<dyn std::error::Error>> {
    let frame_start = Instant::now();
    session.dispatch_input(HtmlBrowserInput::Scroll {
        delta_x: 0.0,
        delta_y: -max_scroll,
    })?;
    let top_scroll = session
        .latest_frame()
        .ok_or("full-Y preparation did not produce a top frame")?
        .scroll_y;
    if top_scroll != 0.0 {
        return Err("full-Y preparation did not return to scroll origin".into());
    }
    Ok(frame_start.elapsed().as_secs_f64() * MILLISECONDS_PER_SECOND)
}

fn pointer_click_inputs(x: f32, y: f32) -> [HtmlBrowserInput; POINTER_EVENT_SEQUENCE_LENGTH] {
    [
        HtmlBrowserInput::PointerMove { x, y },
        HtmlBrowserInput::PointerDown { x, y, button: 0 },
        HtmlBrowserInput::PointerUp { x, y, button: 0 },
    ]
}

#[cfg(test)]
mod tests {
    use super::measure_full_y;
    #[cfg(feature = "baseline")]
    use baseline::{HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};
    #[cfg(feature = "candidate")]
    use candidate::{HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};

    #[test]
    fn full_y_records_bottom_frame_and_returns_real_session_to_top(
    ) -> Result<(), Box<dyn std::error::Error>> {
        for height in [80, 1800] {
            let viewport = HtmlBrowserViewport::new(160, 120, 1.0)?;
            let html = format!(
                "<style>body{{margin:0}}</style><div style='height:{height}px;background:#abcdef'></div>"
            );
            let source = HtmlBrowserSource::new(html, "https://example.test/full-y.html")?;
            let mut session = HtmlRuntime.open(source, viewport)?;
            let (measurement, steps, final_y, max_scroll) = measure_full_y(&mut session, viewport)?;
            assert_eq!(steps > 0, max_scroll > 0.0);
            assert_eq!(final_y, max_scroll);
            assert_eq!(
                measurement["frame"]["scroll_y"].as_f64(),
                Some(f64::from(max_scroll))
            );
            assert_eq!(
                session.latest_frame().map(|frame| frame.scroll_y),
                Some(0.0)
            );
            assert_full_y_measurement(&measurement)?;
            session.close()?;
        }
        Ok(())
    }

    fn assert_full_y_measurement(
        measurement: &serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let elapsed = measurement["elapsed_ms"]
            .as_f64()
            .ok_or("missing elapsed")?;
        let maximum = measurement["max_frame_elapsed_ms"]
            .as_f64()
            .ok_or("missing maximum frame time")?;
        assert!(elapsed.is_finite() && maximum.is_finite());
        assert!(maximum >= 0.0 && elapsed >= maximum);
        assert!(
            measurement["peak_rss_bytes"]
                .as_u64()
                .ok_or("missing RSS")?
                > 0
        );
        Ok(())
    }
}
