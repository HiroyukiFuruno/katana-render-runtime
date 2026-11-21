use resvg::usvg;
use skia_safe::{Path, PathBuilder, PathFillType, Point};

pub(super) fn convert(
    source: &usvg::tiny_skia_path::Path,
    fill_rule: Option<usvg::FillRule>,
) -> Path {
    let fill_type = match fill_rule.unwrap_or(usvg::FillRule::NonZero) {
        usvg::FillRule::NonZero => PathFillType::Winding,
        usvg::FillRule::EvenOdd => PathFillType::EvenOdd,
    };
    let mut builder = PathBuilder::new_with_fill_type(fill_type);
    for segment in source.segments() {
        append_segment(&mut builder, segment);
    }
    /* WHY: tiny-skia-pathのPathが検証済みの有限座標を、変換せずにSkiaへ渡す。 */
    builder.detach()
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

#[cfg(test)]
mod tests {
    use super::{convert, usvg};

    #[test]
    fn skia_path_copies_large_finite_curve_points_without_conversion_error() -> Result<(), String> {
        let extent = f32::MAX / 4.0;
        let mut builder = usvg::tiny_skia_path::PathBuilder::new();
        builder.move_to(-extent, 0.0);
        builder.quad_to(0.0, extent, extent, 0.0);
        builder.cubic_to(extent, -extent, 0.0, -extent, -extent, 0.0);
        builder.close();
        let source = builder.finish().ok_or("finite source path missing")?;
        let converted = convert(&source, Some(usvg::FillRule::EvenOdd));
        assert!(converted.is_finite());
        assert_eq!(converted.fill_type(), skia_safe::PathFillType::EvenOdd);
        assert!(
            source
                .points()
                .iter()
                .map(|point| (point.x, point.y))
                .eq(converted.points().iter().map(|point| (point.x, point.y)))
        );
        Ok(())
    }
}
