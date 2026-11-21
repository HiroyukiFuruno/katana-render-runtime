use super::super::{geometry, render_if_supported, style};
use super::{allocate_pixmap, parse, svg_with_path};
use resvg::usvg;
use skia_safe::{AlphaType, ColorType, ImageInfo};
use tiny_skia::{Pixmap, Transform};

enum VisiblePaint {
    Fill,
    Stroke,
}

#[test]
fn transparent_fill_keeps_opaque_stroke_on_custom_path() -> Result<(), String> {
    assert_custom_path_uses_visible_paint(
        "id=\"opacity-fill\" fill=\"#ff0000\" fill-opacity=\"0\" stroke=\"#0000ff\" stroke-width=\"4\"",
        VisiblePaint::Stroke,
    )
}

#[test]
fn transparent_stroke_keeps_opaque_fill_on_custom_path() -> Result<(), String> {
    assert_custom_path_uses_visible_paint(
        "id=\"opacity-stroke\" fill=\"#ff0000\" stroke=\"#0000ff\" stroke-opacity=\"0\" stroke-width=\"4\"",
        VisiblePaint::Fill,
    )
}

#[test]
fn transparent_fill_and_stroke_leave_the_entire_frame_unchanged() -> Result<(), String> {
    const ATTRIBUTES: &str =
        r##"id="transparent" fill="#ff0000" fill-opacity="0" stroke="#0000ff" stroke-opacity="0""##;
    let svg = svg_with_path(super::SQUARE_PATH, ATTRIBUTES);
    let (tree, mut direct) = parse(&svg)?;
    let Some(usvg::Node::Path(path)) = tree.node_by_id("transparent") else {
        return Err("missing transparent path".to_owned());
    };
    assert!(path.is_visible());
    assert!(path.fill().is_some_and(|fill| fill.opacity().get() == 0.0));
    assert!(
        path.stroke()
            .is_some_and(|stroke| stroke.opacity().get() == 0.0)
    );
    assert!(!render_if_supported(
        path,
        Transform::identity(),
        &mut direct.as_mut()
    )?);
    let expected = allocate_pixmap()?;
    assert_eq!(direct.data(), expected.data());
    let rendered = super::render_path(super::SQUARE_PATH, ATTRIBUTES, Transform::identity())?;
    assert_eq!(rendered.data(), expected.data());
    Ok(())
}

fn assert_custom_path_uses_visible_paint(
    attributes: &str,
    expected_paint: VisiblePaint,
) -> Result<(), String> {
    let svg = svg_with_path("M12 12H52V52H12Z", attributes);
    let (tree, mut actual) = parse(&svg)?;
    let Some(usvg::Node::Path(path)) = tree
        .node_by_id("opacity-fill")
        .or_else(|| tree.node_by_id("opacity-stroke"))
    else {
        return Err("missing opacity regression path".to_owned());
    };
    let supported = render_if_supported(path, Transform::identity(), &mut actual.as_mut())?;
    if !supported {
        return Err("transparent component was rejected by custom path".to_owned());
    }
    let expected = render_direct_paint(path, expected_paint)?;
    if actual.data() != expected.data() {
        return Err("custom path changed the remaining visible paint".to_owned());
    }
    Ok(())
}

fn render_direct_paint(path: &usvg::Path, expected: VisiblePaint) -> Result<Pixmap, String> {
    let paint = match expected {
        VisiblePaint::Fill => style::fill(path.fill(), path.rendering_mode()),
        VisiblePaint::Stroke => style::stroke(path.stroke(), path.rendering_mode()),
    }
    .and_then(|paint| paint)
    .ok_or_else(|| "visible paint was not available for control".to_owned())?;
    let converted = geometry::convert(path.data(), path.fill().map(|fill| fill.rule()));
    let mut rendered = allocate_pixmap()?;
    let dimensions = (rendered.width() as i32, rendered.height() as i32);
    let info = ImageInfo::new(dimensions, ColorType::RGBA8888, AlphaType::Premul, None);
    let mut surface =
        skia_safe::surfaces::wrap_pixels(&info, rendered.data_mut(), info.min_row_bytes(), None)
            .ok_or_else(|| "opacity control surface creation failed".to_owned())?;
    surface.canvas().draw_path(&converted, &paint);
    drop(surface);
    Ok(rendered)
}
