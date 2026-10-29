use super::super::path::PreparedPath;
use resvg::usvg::{Rect, Transform};
use skia_safe::{Paint, PaintStyle, StrokeRec};

pub(super) fn ink_bounds(prepared: &PreparedPath) -> Option<Rect> {
    let path_bounds = prepared.path.bounds();
    if !prepared.path.is_finite() || !path_bounds.is_finite() || path_bounds.is_empty() {
        return None;
    }
    let mut radius = 0.0_f32;
    for paint in [prepared.fill.as_ref(), prepared.stroke.as_ref()]
        .into_iter()
        .flatten()
    {
        radius = radius.max(paint_inflation(paint)?);
    }
    rect_from_f64(
        f64::from(path_bounds.left) - f64::from(radius),
        f64::from(path_bounds.top) - f64::from(radius),
        f64::from(path_bounds.right) + f64::from(radius),
        f64::from(path_bounds.bottom) + f64::from(radius),
    )
}

fn paint_inflation(paint: &Paint) -> Option<f32> {
    if paint.shader().is_some()
        || paint.path_effect().is_some()
        || paint.mask_filter().is_some()
        || paint.image_filter().is_some()
    {
        return None;
    }
    if paint.style() == PaintStyle::Stroke
        && (!paint.stroke_width().is_finite() || paint.stroke_width() <= 0.0)
    {
        return None;
    }
    let radius = StrokeRec::inflation_radius_from_paint_and_style(paint, paint.style());
    (radius.is_finite() && radius >= 0.0).then_some(radius)
}

pub(in crate::markdown::svg_rasterize::composite) fn transform_bounds(
    rect: Rect,
    ts: Transform,
) -> Option<Rect> {
    ts.invert()?;
    let values = [ts.sx, ts.kx, ts.tx, ts.ky, ts.sy, ts.ty];
    if !values.into_iter().all(f32::is_finite) {
        return None;
    }
    let points = [
        (rect.x(), rect.y()),
        (rect.right(), rect.y()),
        (rect.x(), rect.bottom()),
        (rect.right(), rect.bottom()),
    ];
    let mut limits = Extents::empty();
    for (x, y) in points {
        limits.include(x, y, ts);
    }
    rect_from_f64(limits.left, limits.top, limits.right, limits.bottom)
}

struct Extents {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl Extents {
    fn empty() -> Self {
        Self {
            left: f64::INFINITY,
            top: f64::INFINITY,
            right: f64::NEG_INFINITY,
            bottom: f64::NEG_INFINITY,
        }
    }

    fn include(&mut self, x: f32, y: f32, ts: Transform) {
        let x = f64::from(x);
        let y = f64::from(y);
        let (left, right) = coordinate(ts.sx, x, ts.kx, y, ts.tx);
        let (top, bottom) = coordinate(ts.ky, x, ts.sy, y, ts.ty);
        self.left = self.left.min(left);
        self.top = self.top.min(top);
        self.right = self.right.max(right);
        self.bottom = self.bottom.max(bottom);
    }
}

fn coordinate(a: f32, x: f64, b: f32, y: f64, offset: f32) -> (f64, f64) {
    let first = f64::from(a) * x + f64::from(b) * y;
    let lower = (first.next_down() + f64::from(offset)).next_down();
    let upper = (first.next_up() + f64::from(offset)).next_up();
    (lower, upper)
}

fn rect_from_f64(left: f64, top: f64, right: f64, bottom: f64) -> Option<Rect> {
    let [left, top, right, bottom] = [left, top, right, bottom].map(|value| value as f32);
    if ![left, top, right, bottom].into_iter().all(f32::is_finite) {
        return None;
    }
    Rect::from_ltrb(
        left.next_down(),
        top.next_down(),
        right.next_up(),
        bottom.next_up(),
    )
}

pub(super) fn union(left: Option<Rect>, right: Option<Rect>) -> Option<Rect> {
    match (left, right) {
        (Some(a), Some(b)) => Rect::from_ltrb(
            a.x().min(b.x()),
            a.y().min(b.y()),
            a.right().max(b.right()),
            a.bottom().max(b.bottom()),
        ),
        (Some(rect), None) | (None, Some(rect)) => Some(rect),
        (None, None) => None,
    }
}
