use resvg::usvg;
use skia_safe::{
    Contains, PaintStyle, Path, PathBuilder, PathFillType, PathVerb, Point, Rect, RoundOut,
    StrokeRec, Vector,
};
use tiny_skia::{PixmapMut, Transform};

const ZERO: f32 = 0.0;
const OVAL_DIAMETER_FACTOR: f32 = 2.0;

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

pub(super) fn source_oval(
    (cx, cy, rx, ry): (f32, f32, f32, f32),
    fill_type: PathFillType,
) -> Option<Path> {
    if ![cx, cy, rx, ry].into_iter().all(f32::is_finite) || rx <= ZERO || ry <= ZERO {
        return None;
    }
    let rect = Rect::from_xywh(
        cx - rx,
        cy - ry,
        rx * OVAL_DIAMETER_FACTOR,
        ry * OVAL_DIAMETER_FACTOR,
    );
    if !rect.is_finite() || rect.is_empty() {
        return None;
    }
    let oval = Path::oval(rect, None).with_fill_type(fill_type);
    (oval.is_finite() && oval.is_oval().is_some()).then_some(oval)
}

pub(super) fn source_svg_arc(svg_path: &str, fill_type: PathFillType) -> Option<Path> {
    let path = Path::from_svg(svg_path)?.with_fill_type(fill_type);
    (path.is_finite() && path.verbs().contains(&PathVerb::Conic)).then_some(path)
}

pub(super) fn path_bounds_fit_target(
    path: &Path,
    prepared: &super::PreparedPath,
    transform: Transform,
    pixmap: &PixmapMut<'_>,
) -> bool {
    if !axis_aligned_nonzero(transform) || !bounded_translation(transform, pixmap) {
        return false;
    }
    let Some(local_bounds) = local_path_bounds(path, prepared) else {
        return false;
    };
    let (mut device_bounds, stays_rect) = super::skia_matrix(transform).map_rect(local_bounds);
    if !stays_rect || !device_bounds.is_finite() {
        return false;
    }
    /* WHY: SkiaのAAが隣接pixelへ及ぶ場合も描画領域内に収めるため、1px外側まで検査する。 */
    device_bounds.outset(Vector::new(
        AA_OUTSET_DEVICE_PIXELS,
        AA_OUTSET_DEVICE_PIXELS,
    ));
    let integer_bounds: skia_safe::IRect = device_bounds.round_out();
    target_bounds(pixmap).is_some_and(|target| target.contains(integer_bounds))
}

const AA_OUTSET_DEVICE_PIXELS: f32 = 1.0;

fn axis_aligned_nonzero(transform: Transform) -> bool {
    transform.kx == ZERO
        && transform.ky == ZERO
        && transform.sx.is_finite()
        && transform.sy.is_finite()
        && transform.sx != ZERO
        && transform.sy != ZERO
}

fn bounded_translation(transform: Transform, pixmap: &PixmapMut<'_>) -> bool {
    transform.tx.is_finite()
        && transform.ty.is_finite()
        && transform.tx.abs() <= pixmap.width() as f32
        && transform.ty.abs() <= pixmap.height() as f32
}

fn local_path_bounds(path: &Path, prepared: &super::PreparedPath) -> Option<Rect> {
    let mut bounds = *path.bounds();
    if let Some(stroke) = &prepared.stroke {
        if stroke.path_effect().is_some()
            || !stroke.stroke_width().is_finite()
            || stroke.stroke_width() <= ZERO
        {
            return None;
        }
        let inflation =
            StrokeRec::from_paint(stroke, None::<PaintStyle>, None::<f32>).inflation_radius();
        if !inflation.is_finite() || inflation < ZERO {
            return None;
        }
        bounds.outset(Vector::new(inflation, inflation));
    }
    bounds.is_finite().then_some(bounds)
}

fn target_bounds(pixmap: &PixmapMut<'_>) -> Option<skia_safe::IRect> {
    let width = i32::try_from(pixmap.width()).ok()?;
    let height = i32::try_from(pixmap.height()).ok()?;
    Some(skia_safe::IRect::from_xywh(0, 0, width, height))
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
