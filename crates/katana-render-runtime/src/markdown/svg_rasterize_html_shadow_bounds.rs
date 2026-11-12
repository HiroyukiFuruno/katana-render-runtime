use resvg::usvg::Rect;
use tiny_skia::{PixmapMut, Point, Transform};

const BBOX_CORNER_COUNT: usize = 4;

#[derive(Clone, Copy)]
pub(in crate::markdown::svg_rasterize) struct FilterTransform(Transform);

impl FilterTransform {
    pub(in crate::markdown::svg_rasterize) fn new(transform: Transform) -> Option<Self> {
        supported_transform(transform).then_some(Self(transform))
    }

    pub(in crate::markdown::svg_rasterize) fn raw(self) -> Transform {
        self.0
    }
}

#[derive(Clone, Copy)]
pub(in crate::markdown::svg_rasterize) struct Bounds {
    pub(in crate::markdown::svg_rasterize) left: f32,
    pub(in crate::markdown::svg_rasterize) top: f32,
    pub(in crate::markdown::svg_rasterize) right: f32,
    pub(in crate::markdown::svg_rasterize) bottom: f32,
}

#[derive(Clone, Copy)]
pub(in crate::markdown::svg_rasterize) struct Crop {
    pub(in crate::markdown::svg_rasterize) left: i32,
    pub(in crate::markdown::svg_rasterize) top: i32,
    pub(in crate::markdown::svg_rasterize) right: i32,
    pub(in crate::markdown::svg_rasterize) bottom: i32,
}

impl Crop {
    pub(in crate::markdown::svg_rasterize) fn width(self) -> u32 {
        (self.right - self.left) as u32
    }

    pub(in crate::markdown::svg_rasterize) fn height(self) -> u32 {
        (self.bottom - self.top) as u32
    }
}

pub(in crate::markdown::svg_rasterize) fn rect_bounds(rect: Rect) -> Bounds {
    Bounds {
        left: rect.left(),
        top: rect.top(),
        right: rect.right(),
        bottom: rect.bottom(),
    }
}

pub(in crate::markdown::svg_rasterize) fn map_rect(
    rect: Rect,
    transform: FilterTransform,
) -> Result<Bounds, String> {
    map_bounds(rect_bounds(rect), transform)
}

pub(in crate::markdown::svg_rasterize) fn bounded_crop(
    effect: Bounds,
    target: &PixmapMut<'_>,
) -> Option<Crop> {
    let crop = Crop {
        left: effect.left.floor() as i32,
        top: effect.top.floor() as i32,
        right: effect.right.ceil() as i32,
        bottom: effect.bottom.ceil() as i32,
    };
    if crop.left < 0
        || crop.top < 0
        || crop.right > target.width() as i32
        || crop.bottom > target.height() as i32
        || crop.left >= crop.right
        || crop.top >= crop.bottom
    {
        return None;
    }
    Some(crop)
}

pub(in crate::markdown::svg_rasterize) fn map_bounds(
    bounds: Bounds,
    transform: FilterTransform,
) -> Result<Bounds, String> {
    let mut points: [Point; BBOX_CORNER_COUNT] = [
        Point::from_xy(bounds.left, bounds.top),
        Point::from_xy(bounds.right, bounds.top),
        Point::from_xy(bounds.left, bounds.bottom),
        Point::from_xy(bounds.right, bounds.bottom),
    ];
    transform.raw().map_points(&mut points);
    bounds_from_points(&points)
}

fn bounds_from_points(points: &[Point; BBOX_CORNER_COUNT]) -> Result<Bounds, String> {
    let left = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let top = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let right = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    validate_bounds(Bounds {
        left,
        top,
        right,
        bottom,
    })
}

fn validate_bounds(bounds: Bounds) -> Result<Bounds, String> {
    if [bounds.left, bounds.top, bounds.right, bounds.bottom]
        .iter()
        .all(|value| value.is_finite())
    {
        Ok(bounds)
    } else {
        Err("drop-shadow transformed bounds are non-finite".to_owned())
    }
}

fn supported_transform(transform: Transform) -> bool {
    transform.sx.is_finite()
        && transform.sy.is_finite()
        && transform.tx.is_finite()
        && transform.ty.is_finite()
        && transform.sx > 0.0
        && transform.sy > 0.0
        && transform.kx == 0.0
        && transform.ky == 0.0
}

pub(in crate::markdown::svg_rasterize) fn contains(outer: Bounds, inner: Bounds) -> bool {
    outer.left <= inner.left
        && outer.top <= inner.top
        && outer.right >= inner.right
        && outer.bottom >= inner.bottom
}
