use resvg::usvg;
use skia_safe::{Paint, PaintStyle};

use super::solid_color;

const OPAQUE_OPACITY: f32 = 1.0;
const TRANSPARENT_OPACITY: f32 = 0.0;

pub(super) fn fill(
    fill: Option<&usvg::Fill>,
    rendering: usvg::ShapeRendering,
) -> Option<Option<Paint>> {
    let Some(fill) = fill else {
        return Some(None);
    };
    let usvg::Paint::Color(color) = fill.paint() else {
        return None;
    };
    if fill.opacity().get() == TRANSPARENT_OPACITY {
        return Some(None);
    }
    if fill.opacity().get() != OPAQUE_OPACITY {
        return None;
    }
    let mut paint = solid_color(*color);
    paint
        .set_style(PaintStyle::Fill)
        .set_anti_alias(rendering.use_shape_antialiasing());
    Some(Some(paint))
}

pub(super) fn stroke(
    stroke: Option<&usvg::Stroke>,
    rendering: usvg::ShapeRendering,
) -> Option<Option<Paint>> {
    let Some(stroke) = stroke else {
        return Some(None);
    };
    let usvg::Paint::Color(color) = stroke.paint() else {
        return None;
    };
    if stroke.dasharray().is_some() || stroke.linejoin() == usvg::LineJoin::MiterClip {
        return None;
    }
    if stroke.opacity().get() == TRANSPARENT_OPACITY {
        return Some(None);
    }
    if stroke.opacity().get() != OPAQUE_OPACITY {
        return None;
    }
    let join = line_join(stroke.linejoin())?;
    let mut paint = solid_color(*color);
    paint
        .set_style(PaintStyle::Stroke)
        .set_anti_alias(rendering.use_shape_antialiasing())
        .set_stroke_width(stroke.width().get())
        .set_stroke_miter(stroke.miterlimit().get())
        .set_stroke_cap(line_cap(stroke.linecap()))
        .set_stroke_join(join);
    Some(Some(paint))
}

fn line_cap(cap: usvg::LineCap) -> skia_safe::PaintCap {
    match cap {
        usvg::LineCap::Butt => skia_safe::PaintCap::Butt,
        usvg::LineCap::Round => skia_safe::PaintCap::Round,
        usvg::LineCap::Square => skia_safe::PaintCap::Square,
    }
}

fn line_join(join: usvg::LineJoin) -> Option<skia_safe::PaintJoin> {
    match join {
        usvg::LineJoin::Miter => Some(skia_safe::PaintJoin::Miter),
        usvg::LineJoin::Round => Some(skia_safe::PaintJoin::Round),
        usvg::LineJoin::Bevel => Some(skia_safe::PaintJoin::Bevel),
        usvg::LineJoin::MiterClip => None,
    }
}
