use super::{marker_path_count, parse, render_source};
use tiny_skia::Pixmap;

const MARKER: &str = "<defs><marker id='m' markerUnits='userSpaceOnUse' markerWidth='10' markerHeight='10' orient='auto' overflow='visible'><path d='M0 -2 L8 0 L0 2 Z' fill='red'/></marker></defs>";

fn source(path: &str, marker: &str) -> String {
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='80' height='40'>{MARKER}<path id='route' d='{path}' fill='none' stroke='black' {marker}/></svg>"
    )
}

fn assert_red(image: &Pixmap, x: u32, y: u32) -> Result<(), String> {
    let pixel = image.pixel(x, y).ok_or("marker sample outside image")?;
    assert!(
        pixel.red() > pixel.green() && pixel.red() > pixel.blue(),
        "{x},{y}: {pixel:?}"
    );
    Ok(())
}

#[test]
fn move_then_close_middle_markers_preserve_both_tangents() -> Result<(), String> {
    let svg = source("M10 20 L30 20 M50 20 Z", "marker-mid='url(#m)'");
    let tree = parse(&svg)?;
    assert_eq!(marker_path_count(tree.root()), 2);
    let image = render_source(&svg)?;
    assert_red(&image, 33, 20)?;
    assert_red(&image, 53, 20)?;
    assert_eq!(image.pixel(47, 20).ok_or("sample missing")?.alpha(), 0);
    Ok(())
}

#[test]
fn zero_length_closed_subpath_end_marker_uses_its_own_vertex() -> Result<(), String> {
    let svg = source("M10 20 L30 20 M50 20 Z", "marker-end='url(#m)'");
    let tree = parse(&svg)?;
    assert_eq!(marker_path_count(tree.root()), 1);
    let image = render_source(&svg)?;
    assert_red(&image, 53, 20)?;
    assert_eq!(image.pixel(47, 20).ok_or("sample missing")?.alpha(), 0);
    Ok(())
}

#[test]
fn closed_empty_first_subpath_start_marker_has_zero_angle() -> Result<(), String> {
    let svg = source("M10 20 Z L30 20", "marker-start='url(#m)'");
    let tree = parse(&svg)?;
    assert_eq!(marker_path_count(tree.root()), 1);
    let image = render_source(&svg)?;
    assert_red(&image, 13, 20)?;
    assert_eq!(image.pixel(6, 20).ok_or("sample missing")?.alpha(), 0);
    Ok(())
}

#[test]
fn trailing_move_receives_the_path_end_marker() -> Result<(), String> {
    let path = "M10 20 L30 20 M50 20";
    let marked = source(path, "marker-end='url(#m)'");
    let plain = source(path, "");
    assert_eq!(marker_path_count(parse(&marked)?.root()), 1);
    let actual = render_source(&marked)?;
    let expected = render_source(&plain)?;
    assert_red(&actual, 53, 20)?;
    assert_ne!(actual.data(), expected.data());
    assert!(actual.pixels().iter().any(|pixel| pixel.alpha() > 0));
    Ok(())
}

#[test]
fn marker_with_unrepresentable_clip_leaves_the_stroked_path_visible() -> Result<(), String> {
    let path = "M10 20 L30 20";
    let marked = source(path, "marker-start='url(#m)'").replace(
        "markerWidth='10' markerHeight='10' orient='auto' overflow='visible'",
        "markerWidth='1e-39' markerHeight='10' viewBox='0 0 10 10' orient='auto'",
    );
    assert_eq!(marker_path_count(parse(&marked)?.root()), 0);
    let actual = render_source(&marked)?;
    let unmarked = render_source(&source(path, ""))?;
    assert!(unmarked.pixels().iter().any(|pixel| pixel.alpha() > 0));
    assert_eq!(actual.data(), unmarked.data());
    Ok(())
}

#[test]
fn default_marker_overflow_clips_to_marker_viewport() -> Result<(), String> {
    let visible = source("M10 20 L30 20", "marker-start='url(#m)'")
        .replace("markerWidth='10'", "markerWidth='3'");
    let clipped = visible.replace(" overflow='visible'", "");
    assert_eq!(marker_path_count(parse(&clipped)?.root()), 1);
    let clipped_image = render_source(&clipped)?;
    let visible_image = render_source(&visible)?;
    assert_red(&clipped_image, 11, 20)?;
    assert_red(&visible_image, 15, 20)?;
    let clipped_pixel = clipped_image.pixel(15, 20).ok_or("sample missing")?;
    assert!(clipped_pixel.red() <= clipped_pixel.green());
    Ok(())
}

#[test]
fn singular_marker_context_transform_preserves_other_paint() -> Result<(), String> {
    let singular = context_gradient_marker("matrix(1 1 1 1 0 0)", "marker-end='url(#m)'");
    let invertible = context_gradient_marker("matrix(1 0 0 1 0 0)", "marker-end='url(#m)'");
    let unmarked = context_gradient_marker("matrix(1 1 1 1 0 0)", "");
    assert_eq!(marker_path_count(parse(&invertible)?.root()), 1);
    let actual = render_source(&singular)?;
    let control = render_source(&invertible)?;
    let plain = render_source(&unmarked)?;
    assert_eq!(actual.data(), plain.data());
    assert_ne!(control.data(), plain.data());
    for image in [&actual, &control, &plain] {
        let pixel = image.pixel(21, 2).ok_or("green sibling sample missing")?;
        assert_eq!(
            (pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()),
            (0, 255, 0, 255)
        );
    }
    Ok(())
}

fn context_gradient_marker(transform: &str, marker: &str) -> String {
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='16'><defs><linearGradient id='p' gradientUnits='userSpaceOnUse' x1='0' y1='0' x2='24' y2='0'><stop stop-color='red'/><stop offset='1' stop-color='blue'/></linearGradient><marker id='m' markerUnits='userSpaceOnUse' markerWidth='8' markerHeight='8' overflow='visible'><rect width='6' height='6' fill='context-stroke' transform='{transform}'/></marker></defs><path id='route' d='M4 8 L12 8' fill='none' stroke='url(#p)' {marker}/><rect id='sibling' x='20' y='1' width='3' height='3' fill='#00ff00'/></svg>"
    )
}
