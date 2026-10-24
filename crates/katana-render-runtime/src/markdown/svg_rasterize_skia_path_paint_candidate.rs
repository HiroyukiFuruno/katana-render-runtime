use resvg::usvg::{Fill, LineCap, LineJoin, Paint as SvgPaint, ShapeRendering, Stroke};
use skia_safe::{Color4f, Paint, PaintStyle, PathEffect};

const CHANNEL_MAX: f32 = u8::MAX as f32;

pub(super) fn supported_paints(fill: Option<&Fill>, stroke: Option<&Stroke>) -> bool {
    let fill_supported = fill.is_none_or(|value| matches!(value.paint(), SvgPaint::Color(_)));
    let stroke_supported = stroke.is_none_or(|value| {
        matches!(value.paint(), SvgPaint::Color(_))
            && !matches!(value.linejoin(), LineJoin::MiterClip)
            && value.dasharray().is_none_or(|intervals| {
                intervals
                    .iter()
                    .all(|value| value.is_finite() && *value >= 0.0)
                    && intervals.iter().any(|value| *value > 0.0)
            })
    });
    fill_supported && stroke_supported
}

pub(super) fn make_fill(
    fill: Option<&Fill>,
    rendering_mode: ShapeRendering,
) -> Result<Option<Paint>, String> {
    let Some(fill) = fill else {
        return Ok(None);
    };
    let SvgPaint::Color(color) = fill.paint() else {
        return Ok(None);
    };
    Ok(Some(make_paint(
        color.red,
        color.green,
        color.blue,
        fill.opacity().get(),
        PaintStyle::Fill,
        rendering_mode,
        None,
    )?))
}

pub(super) fn make_stroke(
    stroke: Option<&Stroke>,
    rendering_mode: ShapeRendering,
) -> Result<Option<Paint>, String> {
    let Some(stroke) = stroke else {
        return Ok(None);
    };
    let SvgPaint::Color(color) = stroke.paint() else {
        return Ok(None);
    };
    let style = stroke_style(stroke)?;
    let mut paint = make_paint(
        color.red,
        color.green,
        color.blue,
        stroke.opacity().get(),
        PaintStyle::Stroke,
        rendering_mode,
        dash_effect(stroke.dasharray(), stroke.dashoffset())?,
    )?;
    apply_stroke_style(&mut paint, style);
    Ok(Some(paint))
}

struct StrokeStyle {
    cap: skia_safe::paint::Cap,
    join: skia_safe::paint::Join,
    width: f32,
    miter_limit: f32,
}

fn stroke_style(stroke: &Stroke) -> Result<StrokeStyle, String> {
    let cap = match stroke.linecap() {
        LineCap::Butt => skia_safe::paint::Cap::Butt,
        LineCap::Round => skia_safe::paint::Cap::Round,
        LineCap::Square => skia_safe::paint::Cap::Square,
    };
    let join = match stroke.linejoin() {
        LineJoin::Miter => skia_safe::paint::Join::Miter,
        LineJoin::Round => skia_safe::paint::Join::Round,
        LineJoin::Bevel => skia_safe::paint::Join::Bevel,
        LineJoin::MiterClip => return Err("Skia does not support miter-clip joins".to_string()),
    };
    let width = stroke.width().get();
    let miter_limit = stroke.miterlimit().get();
    if !width.is_finite() || width <= 0.0 || !miter_limit.is_finite() || miter_limit <= 0.0 {
        return Err("stroke dimensions must be finite and positive".to_string());
    }
    Ok(StrokeStyle {
        cap,
        join,
        width,
        miter_limit,
    })
}

fn apply_stroke_style(paint: &mut Paint, style: StrokeStyle) {
    paint
        .set_stroke_width(style.width)
        .set_stroke_miter(style.miter_limit)
        .set_stroke_cap(style.cap)
        .set_stroke_join(style.join);
}

fn make_paint(
    red: u8,
    green: u8,
    blue: u8,
    opacity: f32,
    style: PaintStyle,
    rendering_mode: ShapeRendering,
    effect: Option<PathEffect>,
) -> Result<Paint, String> {
    if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
        return Err("paint opacity must be finite and normalized".to_string());
    }
    let color = Color4f {
        r: f32::from(red) / CHANNEL_MAX,
        g: f32::from(green) / CHANNEL_MAX,
        b: f32::from(blue) / CHANNEL_MAX,
        a: opacity,
    };
    let mut paint = Paint::default();
    paint
        .set_color4f(color, None)
        .set_style(style)
        .set_anti_alias(rendering_mode.use_shape_antialiasing())
        .set_path_effect(effect);
    Ok(paint)
}

fn dash_effect(dasharray: Option<&[f32]>, offset: f32) -> Result<Option<PathEffect>, String> {
    let Some(intervals) = dasharray else {
        return Ok(None);
    };
    if intervals.is_empty()
        || !offset.is_finite()
        || intervals
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err("stroke dash values must be finite and non-negative".to_string());
    }
    PathEffect::dash(intervals, offset)
        .map(Some)
        .ok_or_else(|| "Skia rejected the stroke dash effect".to_string())
}
