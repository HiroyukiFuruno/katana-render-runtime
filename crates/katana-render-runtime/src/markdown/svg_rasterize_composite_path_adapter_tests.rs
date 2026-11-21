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
