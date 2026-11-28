use super::{only_path, parse};
use tiny_skia::Pixmap;

#[test]
fn path_adapter_paints_opaque_svg() -> Result<(), String> {
    let tree = parse(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path id="painted" d="M4 4H20V20H4Z" fill="#ff0000" stroke="#0000ff" stroke-width="2"/></svg>"##,
    )?;
    let path = only_path(tree.root())?;
    let mut pixmap = Pixmap::new(24, 24).ok_or("opaque path pixmap allocation failed")?;
    assert!(super::super::render_if_supported(
        path,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    )?);
    let center = pixmap
        .pixel(12, 12)
        .ok_or("opaque path center pixel missing")?;
    assert_eq!(
        [center.red(), center.green(), center.blue(), center.alpha()],
        [255, 0, 0, 255]
    );
    Ok(())
}

#[test]
fn path_adapter_rejects_invalid_or_unsupported_paint() -> Result<(), String> {
    let tree = parse(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path id="painted" d="M4 4H20V20H4Z" fill="#ff0000" stroke="#0000ff" stroke-width="2"/></svg>"##,
    )?;
    let path = only_path(tree.root())?;
    let mut pixmap = Pixmap::new(24, 24).ok_or("opaque path pixmap allocation failed")?;

    let before = pixmap.data().to_vec();
    assert!(!super::super::render_if_supported(
        path,
        tiny_skia::Transform::from_translate(f32::NAN, 0.0),
        &mut pixmap.as_mut(),
    )?);
    assert_eq!(pixmap.data(), before);

    let unsupported = parse(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M4 4H20V20H4Z" fill="#ff0000" fill-opacity="0.5"/></svg>"##,
    )?;
    let unsupported_path = only_path(unsupported.root())?;
    assert!(!super::super::render_if_supported(
        unsupported_path,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    )?);
    assert_eq!(pixmap.data(), before);
    Ok(())
}

#[test]
fn path_adapter_treats_invisible_and_transparent_paint_as_noop() -> Result<(), String> {
    let invisible = parse(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M4 4H20V20H4Z" fill="#ff0000" visibility="hidden"/></svg>"##,
    )?;
    let transparent = parse(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M4 4H20V20H4Z" fill="#ff0000" fill-opacity="0" stroke="#0000ff" stroke-opacity="0"/></svg>"##,
    )?;
    let mut pixmap = Pixmap::new(24, 24).ok_or("noop path pixmap allocation failed")?;
    let before = pixmap.data().to_vec();
    for path in [only_path(invisible.root())?, only_path(transparent.root())?] {
        assert!(!path.is_visible() || super::super::prepare_path(path).is_none());
        let supported = super::super::render_if_supported(
            path,
            tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        )?;
        assert_eq!(supported, !path.is_visible());
        assert_eq!(pixmap.data(), before);
    }
    Ok(())
}

#[test]
fn path_adapter_paints_stroke_before_fill_when_requested() -> Result<(), String> {
    let (supported, pixmap) = render_adapter_svg(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><path d="M12 12H52V52H12Z" fill="#ff0000" stroke="#0000ff" stroke-width="12" paint-order="stroke fill"/></svg>"##,
    )?;
    assert!(supported);
    let overlap = pixmap
        .pixel(14, 20)
        .ok_or("paint-order overlap pixel missing")?;
    assert!(overlap.red() > overlap.blue());
    Ok(())
}

#[test]
fn path_adapter_handles_square_caps_and_bevel_joins() -> Result<(), String> {
    let (square_supported, square_pixels) = render_adapter_svg(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><path d="M16 20H48" fill="none" stroke="#ffffff" stroke-width="8" stroke-linecap="square" shape-rendering="crispEdges"/></svg>"##,
    )?;
    let (bevel_supported, bevel_pixels) = render_adapter_svg(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><path d="M16 48L32 16L48 48" fill="none" stroke="#ffffff" stroke-width="8" stroke-linejoin="bevel" stroke-miterlimit="3"/></svg>"##,
    )?;
    assert!(square_supported && bevel_supported);
    assert!(
        square_pixels
            .pixel(12, 16)
            .is_some_and(|pixel| pixel.alpha() > 0)
    );
    assert!(
        bevel_pixels
            .pixel(32, 9)
            .is_some_and(|pixel| pixel.alpha() == 0)
    );
    Ok(())
}

#[test]
fn path_adapter_rejects_non_solid_stroke_and_miter_clip_join() -> Result<(), String> {
    for svg in [
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><defs><linearGradient id="g"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><path d="M4 4H20V20H4Z" fill="#ff0000" stroke="url(#g)" stroke-width="2"/></svg>"##,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M4 4H20V20H4Z" fill="none" stroke="#00ff00" stroke-width="2" stroke-linejoin="miter-clip"/></svg>"##,
    ] {
        assert!(!render_adapter_svg(svg)?.0);
    }
    Ok(())
}

#[test]
fn path_adapter_reports_short_pixel_storage_before_painting() -> Result<(), String> {
    let tree = parse(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M4 4H20V20H4Z" fill="#ff0000"/></svg>"##,
    )?;
    let path = only_path(tree.root())?;
    let prepared =
        super::super::prepare_path(path).ok_or("opaque path was unexpectedly rejected")?;
    let mut pixels = vec![0; 24 * 24 * 4 - 1];
    assert_eq!(
        super::super::paint_path_pixels(
            (24, 24),
            &mut pixels,
            &prepared,
            tiny_skia::Transform::identity(),
            path.paint_order(),
        )
        .err()
        .as_deref(),
        Some("Skia SVG path surface creation failed")
    );
    assert!(pixels.iter().all(|pixel| *pixel == 0));
    Ok(())
}

fn render_adapter_svg(svg: &str) -> Result<(bool, Pixmap), String> {
    let tree = parse(svg)?;
    let size = tree.size().to_int_size();
    let mut pixmap =
        Pixmap::new(size.width(), size.height()).ok_or("path adapter pixmap allocation failed")?;
    let supported = super::super::render_if_supported(
        only_path(tree.root())?,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    )?;
    Ok((supported, pixmap))
}
