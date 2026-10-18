use super::layout::{DeferredAxisClip, HtmlLayoutRenderer};
use super::layout_scroll_extent::{BOTTOM_LEFT_CORNER, RECT_CORNER_COUNT};
use super::types::OverflowClip;
use tiny_skia::{Point, Transform};

pub(super) fn finalize(renderer: &mut HtmlLayoutRenderer) -> Result<(), String> {
    let pending = std::mem::take(&mut renderer.deferred_axis_clips);
    let mut defs = String::new();
    for descriptor in pending {
        defs.push_str(&render_clip(renderer, descriptor)?);
    }
    if !defs.is_empty() {
        renderer.svg.push_str(&format!("<defs>{defs}</defs>"));
    }
    Ok(())
}

fn render_clip(
    renderer: &HtmlLayoutRenderer,
    descriptor: DeferredAxisClip,
) -> Result<String, String> {
    if active_dimension_is_zero(&descriptor.clip) {
        return Ok(empty_clip(descriptor.id));
    }
    let owner = renderer
        .ownership
        .element_box_indices
        .get(&descriptor.owner_node_id)
        .and_then(|index| renderer.element_boxes.get(*index))
        .ok_or_else(|| format!("missing paint clip owner {}", descriptor.owner_node_id))?;
    let transform = owner_transform(descriptor.source_owner, &owner.transformed_corners)?;
    let world_clip = transform_clip(descriptor.clip, transform)?;
    let path = transformed_clip_path(renderer, &world_clip, transform)?;
    Ok(format!(
        r#"<clipPath id="krr-clip-{}"><path d="{}"/></clipPath>"#,
        descriptor.id, path
    ))
}

fn transformed_clip_path(
    renderer: &HtmlLayoutRenderer,
    clip: &OverflowClip,
    transform: Transform,
) -> Result<String, String> {
    let polygon = super::layout_scroll_extent::clip_viewport_polygon(clip, &viewport(renderer))?;
    if polygon.is_empty() {
        return Ok("M 0 0 Z".to_string());
    }
    let inverse = transform
        .invert()
        .ok_or_else(|| "singular owner transform for paint clip".to_string())?;
    Ok(polygon_path(&local_polygon(polygon, inverse, renderer)?))
}

fn active_dimension_is_zero(clip: &OverflowClip) -> bool {
    (clip.active_axes.0 && clip.width == 0.0) || (clip.active_axes.1 && clip.height == 0.0)
}

fn empty_clip(id: u64) -> String {
    format!(r#"<clipPath id="krr-clip-{id}"><path d="M 0 0 Z"/></clipPath>"#)
}

fn viewport(renderer: &HtmlLayoutRenderer) -> [(f64, f64); RECT_CORNER_COUNT] {
    let x = f64::from(renderer.scroll_x);
    let y = f64::from(renderer.scroll_y);
    let right = f64::from(renderer.scroll_x + renderer.viewport_width);
    let bottom = f64::from(renderer.scroll_y + renderer.viewport_height);
    [(x, y), (right, y), (right, bottom), (x, bottom)]
}

fn transform_clip(mut clip: OverflowClip, transform: Transform) -> Result<OverflowClip, String> {
    for point in &mut clip.transformed_corners {
        let mut mapped = Point::from_xy(point.0, point.1);
        transform.map_point(&mut mapped);
        if !mapped.x.is_finite() || !mapped.y.is_finite() {
            return Err("non-finite transformed paint clip point".to_string());
        }
        *point = (mapped.x, mapped.y);
    }
    Ok(clip)
}

fn local_polygon(
    polygon: Vec<(f64, f64)>,
    inverse: Transform,
    renderer: &HtmlLayoutRenderer,
) -> Result<Vec<(f64, f64)>, String> {
    polygon
        .into_iter()
        .map(|(x, y)| {
            let mut point = Point::from_xy(x as f32, y as f32);
            inverse.map_point(&mut point);
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err("non-finite inverse owner transform point".to_string());
            }
            Ok((
                f64::from(point.x) - f64::from(renderer.scroll_x),
                f64::from(point.y) - f64::from(renderer.scroll_y),
            ))
        })
        .collect()
}

fn owner_transform(
    source: (f32, f32, f32, f32),
    corners: &[(f32, f32); RECT_CORNER_COUNT],
) -> Result<Transform, String> {
    let (x, y, width, height) = source;
    if ![x, y, width, height].iter().all(|value| value.is_finite()) {
        return Err("non-finite owner source geometry for paint clip".to_string());
    }
    let origin = corners[0];
    let basis_x = axis_x(width, height, origin, corners)?;
    let basis_y = axis_y(width, height, origin, corners)?;
    let transform = Transform::from_row(
        basis_x.0,
        basis_x.1,
        basis_y.0,
        basis_y.1,
        origin.0 - basis_x.0 * x - basis_y.0 * y,
        origin.1 - basis_x.1 * x - basis_y.1 * y,
    );
    transform
        .is_finite()
        .then_some(transform)
        .ok_or_else(|| "non-finite owner transform for paint clip".to_string())
}

fn axis_x(
    width: f32,
    height: f32,
    origin: (f32, f32),
    corners: &[(f32, f32); RECT_CORNER_COUNT],
) -> Result<(f32, f32), String> {
    if width != 0.0 {
        return Ok((
            (corners[1].0 - origin.0) / width,
            (corners[1].1 - origin.1) / width,
        ));
    }
    if height == 0.0 {
        return Err("zero-size owner cannot define paint clip transform".to_string());
    }
    let axis = (
        corners[BOTTOM_LEFT_CORNER].0 - origin.0,
        corners[BOTTOM_LEFT_CORNER].1 - origin.1,
    );
    let length = axis.0.hypot(axis.1);
    (length.is_finite() && length > 0.0)
        .then_some((axis.1 / length, -axis.0 / length))
        .ok_or_else(|| "degenerate owner y axis for paint clip transform".to_string())
}

fn axis_y(
    width: f32,
    height: f32,
    origin: (f32, f32),
    corners: &[(f32, f32); RECT_CORNER_COUNT],
) -> Result<(f32, f32), String> {
    if height != 0.0 {
        return Ok((
            (corners[BOTTOM_LEFT_CORNER].0 - origin.0) / height,
            (corners[BOTTOM_LEFT_CORNER].1 - origin.1) / height,
        ));
    }
    if width == 0.0 {
        return Err("zero-size owner cannot define paint clip transform".to_string());
    }
    let axis = (corners[1].0 - origin.0, corners[1].1 - origin.1);
    let length = axis.0.hypot(axis.1);
    (length.is_finite() && length > 0.0)
        .then_some((-axis.1 / length, axis.0 / length))
        .ok_or_else(|| "degenerate owner x axis for paint clip transform".to_string())
}

fn polygon_path(polygon: &[(f64, f64)]) -> String {
    let Some(first) = polygon.first() else {
        return "M 0 0 Z".to_string();
    };
    let mut path = format!("M {} {}", first.0, first.1);
    for point in polygon.iter().skip(1) {
        path.push_str(&format!(" L {} {}", point.0, point.1));
    }
    path.push_str(" Z");
    path
}

#[cfg(test)]
#[path = "layout_axis_clip_tests.rs"]
mod tests;
