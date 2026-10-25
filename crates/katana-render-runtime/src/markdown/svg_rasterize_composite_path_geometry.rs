use resvg::usvg;
use skia_safe::{Path, PathBuilder, PathFillType, Point};

pub(super) fn convert(
    source: &usvg::tiny_skia_path::Path,
    fill_rule: Option<usvg::FillRule>,
) -> Result<Path, String> {
    let fill_type = match fill_rule.unwrap_or(usvg::FillRule::NonZero) {
        usvg::FillRule::NonZero => PathFillType::Winding,
        usvg::FillRule::EvenOdd => PathFillType::EvenOdd,
    };
    let mut builder = PathBuilder::new_with_fill_type(fill_type);
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
