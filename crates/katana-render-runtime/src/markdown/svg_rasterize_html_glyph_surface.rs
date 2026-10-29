pub(super) use super::paint::draw_span;
use resvg::usvg;
use skia_safe::Matrix;
#[cfg(test)]
use tiny_skia::PixmapMut;
use tiny_skia::{IntRect, Point, Transform};

const BBOX_CORNER_COUNT: usize = 4;

pub(super) struct Crop {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) fn text_crop(
    text: &usvg::Text,
    transform: Transform,
    width: u32,
    height: u32,
    baseline_shifts: (f32, f32),
    include_stroke: bool,
) -> Option<Crop> {
    let bounds = transformed_text_bounds(text, transform, baseline_shifts, include_stroke)?;
    crop_from_bounds(bounds, width, height)
}

fn transformed_text_bounds(
    text: &usvg::Text,
    transform: Transform,
    baseline_shifts: (f32, f32),
    include_stroke: bool,
) -> Option<(f32, f32, f32, f32)> {
    let bounds = if include_stroke {
        text.flattened().stroke_bounding_box()
    } else {
        text.flattened().bounding_box()
    };
    if bounds.is_empty() {
        return None;
    }
    let mut points = [
        Point::from_xy(bounds.x(), bounds.y()),
        Point::from_xy(bounds.right(), bounds.y()),
        Point::from_xy(bounds.x(), bounds.bottom()),
        Point::from_xy(bounds.right(), bounds.bottom()),
    ];
    transform.map_points(&mut points);
    Some(extend_baseline_bounds(
        point_bounds(&points),
        transform,
        baseline_shifts,
    ))
}

fn extend_baseline_bounds(
    bounds: (f32, f32, f32, f32),
    transform: Transform,
    shifts: (f32, f32),
) -> (f32, f32, f32, f32) {
    let dx = [transform.kx * shifts.0, transform.kx * shifts.1];
    let dy = [transform.sy * shifts.0, transform.sy * shifts.1];
    (
        bounds.0 + 0.0_f32.min(dx[0]).min(dx[1]),
        bounds.1 + 0.0_f32.min(dy[0]).min(dy[1]),
        bounds.2 + 0.0_f32.max(dx[0]).max(dx[1]),
        bounds.3 + 0.0_f32.max(dy[0]).max(dy[1]),
    )
}

fn point_bounds(points: &[Point; BBOX_CORNER_COUNT]) -> (f32, f32, f32, f32) {
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    (min_x, min_y, max_x, max_y)
}

fn crop_from_bounds(
    (min_x, min_y, max_x, max_y): (f32, f32, f32, f32),
    width: u32,
    height: u32,
) -> Option<Crop> {
    let rect = IntRect::from_ltrb(
        (min_x.floor() as i32).saturating_sub(1),
        (min_y.floor() as i32).saturating_sub(1),
        (max_x.ceil() as i32).saturating_add(1),
        (max_y.ceil() as i32).saturating_add(1),
    )?;
    let target = IntRect::from_ltrb(0, 0, width as i32, height as i32)?;
    let rect = rect.intersect(&target)?;
    Some(Crop {
        x: rect.left(),
        y: rect.top(),
        width: rect.width(),
        height: rect.height(),
    })
}

pub(super) fn skia_matrix(transform: Transform, x: f32, y: f32) -> Matrix {
    Matrix::new_all(
        transform.sx,
        transform.kx,
        transform.tx - x,
        transform.ky,
        transform.sy,
        transform.ty - y,
        0.0,
        0.0,
        1.0,
    )
}

#[cfg(test)]
pub(super) fn composite_rgba(
    pixmap: &mut PixmapMut<'_>,
    pixels: &[u8],
    crop: Crop,
) -> Result<(), String> {
    let Some(source) = tiny_skia::PixmapRef::from_bytes(pixels, crop.width, crop.height) else {
        return Err("tiny-skia text surface import failed".to_string());
    };
    pixmap.draw_pixmap(
        crop.x,
        crop.y,
        source,
        &tiny_skia::PixmapPaint::default(),
        tiny_skia::Transform::identity(),
        None,
    );
    Ok(())
}

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_surface_tests.rs"]
mod tests;
