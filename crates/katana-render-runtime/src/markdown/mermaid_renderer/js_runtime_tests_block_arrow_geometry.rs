use super::super::{
    BLOCK_BOTTOM_BOUND_INDEX, BLOCK_BOUNDS_COMPONENT_COUNT, BLOCK_RIGHT_BOUND_INDEX,
};
use resvg::usvg::roxmltree::Node as SvgNode;
use svgtypes::Transform;

pub(in super::super) fn svg_number_or(
    node: SvgNode<'_, '_>,
    name: &str,
    default: f64,
) -> Result<f64, String> {
    node.attribute(name)
        .map_or(Ok(default), str::parse::<f64>)
        .map_err(|error| error.to_string())
}

pub(in super::super) fn transform_bounds(
    bounds: [f64; BLOCK_BOUNDS_COMPONENT_COUNT],
    value: &str,
) -> Result<[f64; BLOCK_BOUNDS_COMPONENT_COUNT], String> {
    let transform: Transform = value
        .parse()
        .map_err(|error: svgtypes::Error| error.to_string())?;
    let corners = [
        (bounds[0], bounds[1]),
        (bounds[BLOCK_RIGHT_BOUND_INDEX], bounds[1]),
        (bounds[0], bounds[BLOCK_BOTTOM_BOUND_INDEX]),
        (
            bounds[BLOCK_RIGHT_BOUND_INDEX],
            bounds[BLOCK_BOTTOM_BOUND_INDEX],
        ),
    ]
    .map(|(x, y)| transform_point(transform, x, y));
    Ok(point_bounds(corners))
}

fn transform_point(transform: Transform, x: f64, y: f64) -> (f64, f64) {
    (
        transform.a * x + transform.c * y + transform.e,
        transform.b * x + transform.d * y + transform.f,
    )
}

fn point_bounds(
    points: [(f64, f64); BLOCK_BOUNDS_COMPONENT_COUNT],
) -> [f64; BLOCK_BOUNDS_COMPONENT_COUNT] {
    [
        points
            .iter()
            .map(|point| point.0)
            .fold(f64::INFINITY, f64::min),
        points
            .iter()
            .map(|point| point.1)
            .fold(f64::INFINITY, f64::min),
        points
            .iter()
            .map(|point| point.0)
            .fold(f64::NEG_INFINITY, f64::max),
        points
            .iter()
            .map(|point| point.1)
            .fold(f64::NEG_INFINITY, f64::max),
    ]
}
