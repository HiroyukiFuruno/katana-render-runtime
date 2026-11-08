// Copyright 2019 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::svg_engine::resvg::render::Context;

pub fn render(
    path: &crate::svg_engine::usvg::Path,
    blend_mode: tiny_skia::BlendMode,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    if !path.is_visible() {
        return;
    }

    if path.paint_order() == crate::svg_engine::usvg::PaintOrder::FillAndStroke {
        fill_path(path, blend_mode, ctx, transform, pixmap);
        stroke_path(path, blend_mode, ctx, transform, pixmap);
    } else {
        stroke_path(path, blend_mode, ctx, transform, pixmap);
        fill_path(path, blend_mode, ctx, transform, pixmap);
    }
}

pub fn fill_path(
    path: &crate::svg_engine::usvg::Path,
    blend_mode: tiny_skia::BlendMode,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
    let fill = path.fill()?;

    // Horizontal and vertical lines cannot be filled. Skip.
    if path.data().bounds().width() == 0.0 || path.data().bounds().height() == 0.0 {
        return None;
    }

    let rule = match fill.rule() {
        crate::svg_engine::usvg::FillRule::NonZero => tiny_skia::FillRule::Winding,
        crate::svg_engine::usvg::FillRule::EvenOdd => tiny_skia::FillRule::EvenOdd,
    };

    with_path_paint(fill.paint(), || fill.opacity(), blend_mode, ctx, transform, path, |paint| {
        pixmap.fill_path(path.data(), paint, rule, transform, None);
    })
}

fn stroke_path(
    path: &crate::svg_engine::usvg::Path,
    blend_mode: tiny_skia::BlendMode,
    ctx: &Context,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
    let stroke = path.stroke()?;
    with_path_paint(stroke.paint(), || stroke.opacity(), blend_mode, ctx, transform, path, |paint| {
        pixmap.stroke_path(path.data(), paint, &stroke.to_tiny_skia(), transform, None);
    })
}

fn with_path_paint(
    source: &crate::svg_engine::usvg::Paint,
    opacity: impl Fn() -> crate::svg_engine::usvg::Opacity,
    blend_mode: tiny_skia::BlendMode,
    ctx: &Context,
    transform: tiny_skia::Transform,
    path: &crate::svg_engine::usvg::Path,
    draw: impl FnOnce(&tiny_skia::Paint),
) -> Option<()> {
    let pattern_pixmap;
    let mut paint = tiny_skia::Paint::default();
    match source {
        crate::svg_engine::usvg::Paint::Color(c) => {
            paint.set_color_rgba8(c.red, c.green, c.blue, opacity().to_u8());
        }
        crate::svg_engine::usvg::Paint::LinearGradient(lg) => {
            paint.shader = convert_linear_gradient(lg, opacity())?;
        }
        crate::svg_engine::usvg::Paint::RadialGradient(rg) => {
            paint.shader = convert_radial_gradient(rg, opacity())?;
        }
        crate::svg_engine::usvg::Paint::Pattern(pattern) => {
            let (patt_pix, patt_ts) = render_pattern_pixmap(pattern, ctx, transform)?;

            pattern_pixmap = patt_pix;
            paint.shader = path_pattern_shader(pattern_pixmap.as_ref(), opacity(), patt_ts);
        }
    }
    paint.anti_alias = path.rendering_mode().use_shape_antialiasing();
    paint.blend_mode = blend_mode;

    draw(&paint);
    Some(())
}

fn path_pattern_shader(
    pixmap: tiny_skia::PixmapRef<'_>,
    opacity: crate::svg_engine::usvg::Opacity,
    transform: tiny_skia::Transform,
) -> tiny_skia::Shader<'_> {
    tiny_skia::Pattern::new(
        pixmap,
        tiny_skia::SpreadMode::Repeat,
        tiny_skia::FilterQuality::Bicubic,
        opacity.get(),
        transform,
    )
}

fn convert_linear_gradient(
    gradient: &crate::svg_engine::usvg::LinearGradient,
    opacity: crate::svg_engine::usvg::Opacity,
) -> Option<tiny_skia::Shader<'_>> {
    let (mode, points) = convert_base_gradient(gradient, opacity)?;

    let shader = tiny_skia::LinearGradient::new(
        (gradient.x1(), gradient.y1()).into(),
        (gradient.x2(), gradient.y2()).into(),
        points,
        mode,
        gradient.transform(),
    )?;

    Some(shader)
}

fn convert_radial_gradient(
    gradient: &crate::svg_engine::usvg::RadialGradient,
    opacity: crate::svg_engine::usvg::Opacity,
) -> Option<tiny_skia::Shader<'_>> {
    let (mode, points) = convert_base_gradient(gradient, opacity)?;

    let shader = tiny_skia::RadialGradient::new(
        (gradient.fx(), gradient.fy()).into(),
        gradient.fr().get(),
        (gradient.cx(), gradient.cy()).into(),
        gradient.r().get(),
        points,
        mode,
        gradient.transform(),
    )?;

    Some(shader)
}

fn convert_base_gradient(
    gradient: &crate::svg_engine::usvg::BaseGradient,
    opacity: crate::svg_engine::usvg::Opacity,
) -> Option<(tiny_skia::SpreadMode, Vec<tiny_skia::GradientStop>)> {
    let mode = match gradient.spread_method() {
        crate::svg_engine::usvg::SpreadMethod::Pad => tiny_skia::SpreadMode::Pad,
        crate::svg_engine::usvg::SpreadMethod::Reflect => tiny_skia::SpreadMode::Reflect,
        crate::svg_engine::usvg::SpreadMethod::Repeat => tiny_skia::SpreadMode::Repeat,
    };

    let mut points = Vec::with_capacity(gradient.stops().len());
    for stop in gradient.stops() {
        let alpha = stop.opacity() * opacity;
        let color = tiny_skia::Color::from_rgba8(
            stop.color().red,
            stop.color().green,
            stop.color().blue,
            alpha.to_u8(),
        );
        points.push(tiny_skia::GradientStop::new(stop.offset().get(), color));
    }

    Some((mode, points))
}

fn render_pattern_pixmap(
    pattern: &crate::svg_engine::usvg::Pattern,
    ctx: &Context,
    transform: tiny_skia::Transform,
) -> Option<(tiny_skia::Pixmap, tiny_skia::Transform)> {
    let (sx, sy) = {
        let ts2 = transform.pre_concat(pattern.transform());
        ts2.get_scale()
    };

    let rect = pattern.rect();
    let img_size = tiny_skia::IntSize::from_wh(
        (rect.width() * sx).round() as u32,
        (rect.height() * sy).round() as u32,
    )?;
    let mut pixmap = tiny_skia::Pixmap::new(img_size.width(), img_size.height())?;

    let transform = tiny_skia::Transform::from_scale(sx, sy);
    crate::svg_engine::resvg::render::render_nodes(pattern.root(), ctx, transform, &mut pixmap.as_mut());

    let mut ts = tiny_skia::Transform::default();
    ts = ts.pre_concat(pattern.transform());
    ts = ts.pre_translate(rect.x(), rect.y());
    ts = ts.pre_scale(1.0 / sx, 1.0 / sy);

    Some((pixmap, ts))
}
