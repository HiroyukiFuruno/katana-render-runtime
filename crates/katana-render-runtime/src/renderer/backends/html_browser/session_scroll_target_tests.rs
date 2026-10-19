use super::*;

#[test]
fn scrolled_absolute_target_accepts_css_coordinate_and_resize_preserves_dom_state() -> TestResult {
    let mut session = open_scroll_session(DEVICE_SCALE_FACTOR)?;
    scroll_to_target(&mut session)?;
    click_target(&mut session)?;
    resize_and_assert_dom_state(&mut session)?;
    Ok(())
}

fn scroll_to_target(session: &mut HtmlBrowserSession) -> TestResult {
    session
        .take_frame_update_with_metrics()
        .ok_or("initial update missing")?;
    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: PHYSICAL_SCROLL_DELTA_X,
            delta_y: PHYSICAL_SCROLL_DELTA_Y,
        })
        .map_err(|error| error.to_string())?;
    assert_eq!(
        pixel_at_content_css(session, TARGET_CONTENT_X, TARGET_CONTENT_Y)?,
        BLUE
    );
    Ok(())
}

fn click_target(session: &mut HtmlBrowserSession) -> TestResult {
    for event in [
        HtmlBrowserInput::PointerDown {
            x: TARGET_VIEWPORT_X,
            y: TARGET_VIEWPORT_Y,
            button: 0,
        },
        HtmlBrowserInput::PointerUp {
            x: TARGET_VIEWPORT_X,
            y: TARGET_VIEWPORT_Y,
            button: 0,
        },
    ] {
        session
            .dispatch_input(event)
            .map_err(|error| error.to_string())?;
    }
    assert_eq!(
        pixel_at_content_css(session, TARGET_CONTENT_X, TARGET_CONTENT_Y)?,
        RED
    );
    Ok(())
}

fn resize_and_assert_dom_state(session: &mut HtmlBrowserSession) -> TestResult {
    let resized =
        HtmlBrowserViewport::new(RESIZED_PHYSICAL_WIDTH, PHYSICAL_HEIGHT, DEVICE_SCALE_FACTOR)
            .map_err(|error| error.to_string())?;
    session.resize(resized).map_err(|error| error.to_string())?;
    assert_resized_frame_metrics(session, resized)?;
    let metrics = required_metrics(session)?;
    assert_eq!(metrics.scroll_x, 0.0);
    assert_eq!(metrics.scroll_y, 300.0);
    assert_eq!(
        pixel_at_content_css(session, TARGET_CONTENT_X, TARGET_CONTENT_Y)?,
        RED
    );
    let resized_frame = session.latest_frame().ok_or("resized frame missing")?;
    assert_eq!(pixel(resized_frame, 8, 8)?, BLUE);
    Ok(())
}

fn assert_resized_frame_metrics(
    session: &HtmlBrowserSession,
    resized: HtmlBrowserViewport,
) -> TestResult {
    let (resized_frame, metrics) = session
        .latest_frame_with_metrics()
        .ok_or("resized frame and metrics must be paired")?;
    assert_resized_viewport(resized_frame, metrics, resized);
    assert_resized_scroll_metrics(metrics);
    Ok(())
}

fn assert_resized_viewport(
    frame: &HtmlBrowserFrame,
    metrics: &HtmlBrowserScrollMetrics,
    resized: HtmlBrowserViewport,
) {
    assert_eq!(frame.generation, metrics.generation);
    assert_eq!(frame.viewport, resized);
    assert_eq!(
        (frame.viewport.width, frame.viewport.height),
        (RESIZED_PHYSICAL_WIDTH, PHYSICAL_HEIGHT)
    );
    assert_eq!(
        (
            frame.viewport.logical_width(),
            frame.viewport.logical_height()
        ),
        (800.0, 240.0)
    );
    assert_eq!(
        (metrics.viewport_width, metrics.viewport_height),
        (800.0, 240.0)
    );
}

fn assert_resized_scroll_metrics(metrics: &HtmlBrowserScrollMetrics) {
    assert_eq!(
        (metrics.content_width, metrics.content_height),
        (800.0, 600.0)
    );
    assert_eq!(
        (metrics.overflow_x, metrics.overflow_y),
        (
            HtmlBrowserOverflowPolicy::Scroll,
            HtmlBrowserOverflowPolicy::Scroll
        )
    );
    assert_eq!(
        (metrics.max_scroll_x(), metrics.max_scroll_y()),
        (0.0, 360.0)
    );
}
