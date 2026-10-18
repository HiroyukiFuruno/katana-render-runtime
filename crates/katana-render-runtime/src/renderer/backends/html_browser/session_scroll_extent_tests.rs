use super::*;

const EXTENT_SCALES: [f32; 2] = [1.0, 2.0];
const EXTENT_EPSILON: f32 = 0.01;

fn assert_extent(
    source: &str,
    expected_width: f32,
    expected_height: f32,
    tolerance: f32,
) -> TestResult {
    for scale in EXTENT_SCALES {
        let (generation, metrics) = extent_frame(source, scale)?;
        assert_extent_metrics(
            generation,
            metrics,
            expected_width,
            expected_height,
            tolerance,
        );
    }
    Ok(())
}

fn extent_frame(source: &str, scale: f32) -> TestResult<(u64, HtmlBrowserScrollMetrics)> {
    let source =
        HtmlBrowserSource::new(source, SOURCE_ORIGIN).map_err(|error| error.to_string())?;
    let session = HtmlBrowserSession::new(source, viewport_for_scale(scale)?)
        .map_err(|error| error.to_string())?;
    session
        .latest_frame_with_metrics()
        .map(|(frame, metrics)| (frame.generation, *metrics))
        .ok_or_else(|| "initial frame and metrics must be paired".to_string())
}

fn assert_extent_metrics(
    frame_generation: u64,
    metrics: HtmlBrowserScrollMetrics,
    expected_width: f32,
    expected_height: f32,
    tolerance: f32,
) {
    assert_eq!(frame_generation, metrics.generation);
    assert_eq!(metrics.viewport_width, LOGICAL_WIDTH);
    assert_eq!(metrics.viewport_height, LOGICAL_HEIGHT);
    assert!((metrics.content_width - expected_width).abs() <= tolerance);
    assert!((metrics.content_height - expected_height).abs() <= tolerance);
    let expected_max_x = (expected_width.round() - LOGICAL_WIDTH).max(0.0);
    let expected_max_y = (expected_height.round() - LOGICAL_HEIGHT).max(0.0);
    assert_eq!(metrics.max_scroll_x(), expected_max_x);
    assert_eq!(metrics.max_scroll_y(), expected_max_y);
}

#[test]
fn nested_horizontal_clip_keeps_vertical_scrollable_extent() -> TestResult {
    assert_extent(
        r#"<style>html,body{margin:0}#clip{width:100px;height:40px;overflow-x:clip;overflow-y:visible}#child{width:20px;height:400px}</style><div id=clip><div id=child></div></div>"#,
        320.0,
        400.0,
        0.0,
    )
}

#[test]
fn nested_vertical_clip_keeps_horizontal_scrollable_extent() -> TestResult {
    assert_extent(
        r#"<style>html,body{margin:0}#clip{width:40px;height:100px;overflow-x:visible;overflow-y:clip}#child{width:500px;height:20px}</style><div id=clip><div id=child></div></div>"#,
        500.0,
        240.0,
        0.0,
    )
}

#[test]
fn fixed_content_outside_viewport_does_not_expand_scrollable_extent() -> TestResult {
    assert_extent(
        r#"<style>html,body{margin:0}#fixed{position:fixed;left:500px;top:500px;width:100px;height:100px}</style><div id=fixed></div>"#,
        320.0,
        240.0,
        0.0,
    )
}

#[test]
fn negative_content_crossing_origin_contributes_positive_extent() -> TestResult {
    assert_extent(
        r#"<style>html,body{margin:0}#negative{position:absolute;left:-50px;top:0;width:400px;height:20px}</style><div id=negative></div>"#,
        350.0,
        240.0,
        0.0,
    )
}

#[test]
fn rotated_content_uses_axis_aligned_scrollable_extent() -> TestResult {
    assert_extent(
        r#"<style>html,body{margin:0}#rotated{position:absolute;left:250px;top:0;width:100px;height:40px;transform:rotate(45deg);transform-origin:center}</style><div id=rotated></div>"#,
        300.0 + 70.0_f32 / 2.0_f32.sqrt(),
        240.0,
        EXTENT_EPSILON,
    )
}

#[test]
fn rotated_horizontal_clip_keeps_only_local_y_overflow() -> TestResult {
    for (rotation, expected_width, expected_height, tolerance) in
        [(-90, 430.0, 240.0, 0.0), (-45, 354.0, 324.0, 0.5)]
    {
        assert_extent(
            &format!(
                r#"<style>html,body{{margin:0;padding:0}}#parent{{width:100px;height:40px;overflow-x:clip;overflow-y:visible;transform:rotate({rotation}deg)}}#child{{width:500px;height:400px}}</style><div id=parent><div id=child></div></div>"#
            ),
            expected_width,
            expected_height,
            tolerance,
        )?;
    }
    Ok(())
}

#[test]
fn rotated_vertical_clip_keeps_only_local_x_overflow() -> TestResult {
    for (rotation, expected_width, expected_height, tolerance) in
        [(90, 320.0, 470.0, 0.0), (45, 382.0, 352.0, 0.5)]
    {
        assert_extent(
            &format!(
                r#"<style>html,body{{margin:0;padding:0}}#parent{{width:100px;height:40px;overflow-x:visible;overflow-y:clip;transform:rotate({rotation}deg)}}#child{{width:500px;height:400px}}</style><div id=parent><div id=child></div></div>"#
            ),
            expected_width,
            expected_height,
            tolerance,
        )?;
    }
    Ok(())
}
