use super::{ALPHA_CHANNEL, HEIGHT, RGBA_CHANNELS, STROKE, UNDERLINE, WIDTH, render, render_sized};
use tiny_skia::{Pixmap, Transform};

const BOUNDS_COMPONENTS: usize = 4;
const MIN_X: usize = 0;
const MIN_Y: usize = 1;
const MAX_X: usize = 2;
const MAX_Y: usize = 3;
const RED: usize = 0;
const BLUE: usize = 2;

#[test]
fn scaled_stroke_and_underline_keep_device_bounds() -> Result<(), String> {
    let style = format!("{STROKE} {UNDERLINE}");
    let unscaled = render_sized(&style, Transform::identity(), WIDTH * 2, HEIGHT * 2)?;
    let scaled = render_sized(
        &style,
        Transform::from_scale(2.0, 2.0),
        WIDTH * 2,
        HEIGHT * 2,
    )?;
    let a = bounds_2d(&unscaled).ok_or("unscaled bounds missing")?;
    let b = bounds_2d(&scaled).ok_or("scaled bounds missing")?;
    assert_scaled_bounds(a, b);
    let stroke = render_sized(STROKE, Transform::identity(), WIDTH * 2, HEIGHT * 2)?;
    let scaled_stroke = render_sized(
        STROKE,
        Transform::from_scale(2.0, 2.0),
        WIDTH * 2,
        HEIGHT * 2,
    )?;
    assert_scaled_bounds(
        bounds_2d(&stroke).ok_or("stroke bounds missing")?,
        bounds_2d(&scaled_stroke).ok_or("scaled stroke bounds missing")?,
    );
    let a = difference_bounds(&stroke, &unscaled).ok_or("underline missing")?;
    let b = difference_bounds(&scaled_stroke, &scaled).ok_or("scaled underline missing")?;
    assert_scaled_bounds(a, b);
    Ok(())
}

#[test]
fn glyph_fill_and_stroke_follow_svg_paint_order() -> Result<(), String> {
    let fill_first = render(
        "style='fill:#f00;stroke:#00f;stroke-width:6;paint-order:fill stroke'",
        Transform::identity(),
    )?;
    let stroke_first = render(
        "style='fill:#f00;stroke:#00f;stroke-width:6;paint-order:stroke fill'",
        Transform::identity(),
    )?;
    assert!(
        fill_first
            .data()
            .chunks_exact(RGBA_CHANNELS)
            .zip(stroke_first.data().chunks_exact(RGBA_CHANNELS))
            .any(|(a, b)| { a[BLUE] > 200 && a[RED] < 30 && b[RED] > 200 && b[BLUE] < 30 })
    );
    Ok(())
}

fn assert_scaled_bounds(a: [u32; BOUNDS_COMPONENTS], b: [u32; BOUNDS_COMPONENTS]) {
    for (original, scaled) in a.into_iter().zip(b) {
        assert!(scaled.abs_diff(original * 2) <= 2, "bounds {a:?} -> {b:?}");
    }
}

fn bounds_2d(pixmap: &Pixmap) -> Option<[u32; BOUNDS_COMPONENTS]> {
    let width = pixmap.width();
    let mut bounds = [width, pixmap.height(), 0, 0];
    let mut found = false;
    for (i, pixel) in pixmap.data().chunks_exact(RGBA_CHANNELS).enumerate() {
        if pixel[ALPHA_CHANNEL] != 0 {
            expand_bounds(&mut bounds, i as u32, width);
            found = true;
        }
    }
    found.then_some(bounds)
}

fn difference_bounds(a: &Pixmap, b: &Pixmap) -> Option<[u32; BOUNDS_COMPONENTS]> {
    let width = a.width();
    let mut bounds = [width, a.height(), 0, 0];
    let mut found = false;
    for (i, (left, right)) in a
        .data()
        .chunks_exact(RGBA_CHANNELS)
        .zip(b.data().chunks_exact(RGBA_CHANNELS))
        .enumerate()
    {
        if left != right {
            expand_bounds(&mut bounds, i as u32, width);
            found = true;
        }
    }
    found.then_some(bounds)
}

fn expand_bounds(bounds: &mut [u32; BOUNDS_COMPONENTS], index: u32, width: u32) {
    let (x, y) = (index % width, index / width);
    bounds[MIN_X] = bounds[MIN_X].min(x);
    bounds[MIN_Y] = bounds[MIN_Y].min(y);
    bounds[MAX_X] = bounds[MAX_X].max(x);
    bounds[MAX_Y] = bounds[MAX_Y].max(y);
}
