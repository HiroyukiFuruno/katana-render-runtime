use super::{
    BACKGROUND_GRAY, CANVAS_SIZE, HtmlShadowRasterizer, Pixmap, SRGB_SVG, Transform, color,
    find_group, paint_blue_source, parse_shadow_svg,
};

#[test]
fn shadow_source_paint_failure_preserves_destination() -> Result<(), String> {
    const SOURCE_ERROR: &str = "shadow source renderer failed";
    let tree = parse_shadow_svg(SRGB_SVG)?;
    let group = find_group(&tree)?;
    let mut target = Pixmap::new(CANVAS_SIZE, CANVAS_SIZE).ok_or("target allocation")?;
    target.fill(color(BACKGROUND_GRAY));
    let before = target.data().to_vec();
    let error = HtmlShadowRasterizer::render(
        group,
        Transform::from_scale(2.0, 2.0),
        &mut target.as_mut(),
        &mut |transform, pixels| {
            paint_blue_source(transform, pixels)?;
            assert!(pixels.data_mut().iter().any(|byte| *byte != 0));
            Err(SOURCE_ERROR.to_owned())
        },
    );
    assert_eq!(error.err().as_deref(), Some(SOURCE_ERROR));
    assert_eq!(target.data(), before.as_slice());
    Ok(())
}

#[test]
fn overflowing_object_bbox_shadow_offset_is_rejected_before_paint() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">
      <defs><filter id="shadow" filterUnits="userSpaceOnUse" primitiveUnits="objectBoundingBox" x="0" y="0" width="20" height="20" color-interpolation-filters="sRGB">
        <feDropShadow dx="1e38" dy="0" stdDeviation="0"/>
      </filter></defs><g filter="url(#shadow)"><rect width="10" height="10" fill="blue"/></g>
    </svg>"##;
    assert_shadow_error_before_paint(
        source,
        Transform::identity(),
        "drop-shadow contains a non-finite parameter",
    )
}

#[test]
fn overflowing_source_bounds_are_rejected_before_paint() -> Result<(), String> {
    assert_shadow_error_before_paint(
        SRGB_SVG,
        Transform::from_scale(f32::MAX, 1.0),
        "drop-shadow transformed bounds are non-finite",
    )
}

fn assert_shadow_error_before_paint(
    svg: &str,
    transform: Transform,
    expected: &str,
) -> Result<(), String> {
    let tree = parse_shadow_svg(svg)?;
    let mut target = Pixmap::new(CANVAS_SIZE, CANVAS_SIZE).ok_or("target allocation")?;
    target.fill(color(BACKGROUND_GRAY));
    let before = target.data().to_vec();
    let mut calls = 0;
    let result = HtmlShadowRasterizer::render(
        find_group(&tree)?,
        transform,
        &mut target.as_mut(),
        &mut |_, _| {
            calls += 1;
            Ok(())
        },
    );
    assert_eq!(result.err().as_deref(), Some(expected));
    assert_eq!(calls, 0);
    assert_eq!(target.data(), before.as_slice());
    Ok(())
}
