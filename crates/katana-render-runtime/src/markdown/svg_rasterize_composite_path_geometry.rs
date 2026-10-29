use resvg::usvg;
use skia_safe::{Path, PathBuilder, PathFillType, Point};

#[cfg(test)]
#[path = "svg_rasterize_composite_path_geometry_tests.rs"]
mod tests;

pub(super) fn convert_node(node: &usvg::Path) -> Result<Path, String> {
    let fill_rule = node.fill().map(|value| value.rule());
    if let Some(path) = node
        .original_arc()
        .and_then(|svg| parse_original(svg, fill_rule))
        && bounds_fit(&path, node.bounding_box())
    {
        return Ok(path);
    }
    convert(node.data(), fill_rule)
}

fn bounds_fit(path: &Path, bounds: usvg::Rect) -> bool {
    let actual = path.compute_tight_bounds();
    actual.left >= bounds.x()
        && actual.top >= bounds.y()
        && actual.right <= bounds.right()
        && actual.bottom <= bounds.bottom()
}

pub(super) fn parse_original(svg: &str, fill_rule: Option<usvg::FillRule>) -> Option<Path> {
    if svg.contains('\0') {
        return None;
    }
    let mut path = Path::from_svg(svg)?;
    path.set_fill_type(fill_type(fill_rule));
    path.is_finite().then_some(path)
}

fn fill_type(fill_rule: Option<usvg::FillRule>) -> PathFillType {
    match fill_rule.unwrap_or(usvg::FillRule::NonZero) {
        usvg::FillRule::NonZero => PathFillType::Winding,
        usvg::FillRule::EvenOdd => PathFillType::EvenOdd,
    }
}

pub(super) fn convert(
    source: &usvg::tiny_skia_path::Path,
    fill_rule: Option<usvg::FillRule>,
) -> Result<Path, String> {
    let mut builder = PathBuilder::new_with_fill_type(fill_type(fill_rule));
    for segment in source.segments() {
        append_segment(&mut builder, segment);
    }
    let path = builder.detach();
    if path.is_finite() {
        Ok(path)
    } else {
        Err("Skia SVG path contains non-finite geometry".to_owned())
    }
}

fn append_segment(builder: &mut PathBuilder, segment: usvg::tiny_skia_path::PathSegment) {
    match segment {
        usvg::tiny_skia_path::PathSegment::MoveTo(point) => {
            builder.move_to(skia_point(point));
        }
        usvg::tiny_skia_path::PathSegment::LineTo(point) => {
            builder.line_to(skia_point(point));
        }
        usvg::tiny_skia_path::PathSegment::QuadTo(control, point) => {
            builder.quad_to(skia_point(control), skia_point(point));
        }
        usvg::tiny_skia_path::PathSegment::CubicTo(first, second, point) => {
            builder.cubic_to(skia_point(first), skia_point(second), skia_point(point));
        }
        usvg::tiny_skia_path::PathSegment::Close => {
            builder.close();
        }
    }
}

fn skia_point(point: usvg::tiny_skia_path::Point) -> Point {
    Point::new(point.x, point.y)
}
