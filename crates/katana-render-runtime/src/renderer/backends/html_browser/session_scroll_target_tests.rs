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
    let metrics = required_metrics(session)?;
    assert_eq!(metrics.viewport_width, 800.0);
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
