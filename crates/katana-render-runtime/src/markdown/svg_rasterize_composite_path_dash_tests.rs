use super::super::{geometry, render_if_supported, style};
use super::{allocate_pixmap, parse, svg_with_path};
use resvg::usvg;
use skia_safe::{AlphaType, ColorType, ImageInfo, Paint, PaintStyle};
use tiny_skia::{Pixmap, Transform};

const DASH_INTERVAL: f32 = 7.0;
const DASH_STROKE_WIDTH: f32 = 6.0;

#[test]
fn opaque_dash_stroke_matches_skia_path_effect_for_both_offsets() -> Result<(), String> {
    for (offset, paint_order, transform) in [
        ("3", "fill stroke", Transform::identity()),
        (
            "-3",
            "stroke fill",
            Transform::from_row(0.96, 0.11, -0.07, 0.98, 3.25, 4.5),
        ),
    ] {
        let attributes = format!(
            "id=\"dash\" {} stroke=\"#0000ff\" stroke-width=\"{DASH_STROKE_WIDTH}\" stroke-linecap=\"butt\" stroke-linejoin=\"miter\" stroke-dasharray=\"{DASH_INTERVAL} {DASH_INTERVAL}\" stroke-dashoffset=\"{offset}\" paint-order=\"{paint_order}\"",
            if offset == "3" {
                "fill=\"none\""
            } else {
                "fill=\"#ff0000\""
            }
        );
        assert_dash_matches_direct_control(&attributes, transform)?;
    }
    Ok(())
}

fn assert_dash_matches_direct_control(
    attributes: &str,
    transform: Transform,
) -> Result<(), String> {
    let svg = svg_with_path("M8 20H56V44H8Z", attributes);
    let (tree, mut actual) = parse(&svg)?;
    let Some(usvg::Node::Path(path)) = tree.node_by_id("dash") else {
        return Err("missing dash regression path".to_owned());
    };
    let supported = render_if_supported(path, transform, &mut actual.as_mut())?;
    if !supported {
        return Err("opaque dash stroke was rejected by custom path".to_owned());
    }
    let expected = render_direct_control(path, transform)?;
    if actual.data() != expected.data() {
        return Err("custom dash stroke differs from Skia path effect".to_owned());
    }
    Ok(())
}

fn render_direct_control(path: &usvg::Path, transform: Transform) -> Result<Pixmap, String> {
    let converted = geometry::convert(path.data(), path.fill().map(|fill| fill.rule()));
    let fill = style::fill(path.fill(), path.rendering_mode())
        .ok_or_else(|| "dash control rejected fill".to_owned())?;
    let stroke = dash_stroke(path)?;
    let mut rendered = allocate_pixmap()?;
    let dimensions = (rendered.width() as i32, rendered.height() as i32);
    let info = ImageInfo::new(dimensions, ColorType::RGBA8888, AlphaType::Premul, None);
    let mut surface =
        skia_safe::surfaces::wrap_pixels(&info, rendered.data_mut(), info.min_row_bytes(), None)
            .ok_or_else(|| "dash control surface creation failed".to_owned())?;
    surface
        .canvas()
        .set_matrix(&skia_safe::M44::from(&super::super::skia_matrix(transform)));
    let paints = match path.paint_order() {
        usvg::PaintOrder::FillAndStroke => (fill.as_ref(), Some(&stroke)),
        usvg::PaintOrder::StrokeAndFill => (Some(&stroke), fill.as_ref()),
    };
    for paint in [paints.0, paints.1].into_iter().flatten() {
        surface.canvas().draw_path(&converted, paint);
    }
    drop(surface);
    Ok(rendered)
}

fn dash_stroke(path: &usvg::Path) -> Result<Paint, String> {
    let Some(stroke) = path.stroke() else {
        return Err("dash control missing stroke".to_owned());
    };
    let usvg::Paint::Color(color) = stroke.paint() else {
        return Err("dash control requires solid stroke".to_owned());
    };
    let intervals = stroke
        .dasharray()
        .ok_or_else(|| "dash control missing intervals".to_owned())?;
    let effect = skia_safe::PathEffect::dash(intervals, stroke.dashoffset())
        .ok_or_else(|| "failed to create Skia dash effect".to_owned())?;
    let mut paint = super::super::solid_color(*color);
    paint
        .set_style(PaintStyle::Stroke)
        .set_anti_alias(path.rendering_mode().use_shape_antialiasing())
        .set_stroke_width(stroke.width().get())
        .set_stroke_miter(stroke.miterlimit().get())
        .set_stroke_cap(skia_safe::PaintCap::Butt)
        .set_stroke_join(skia_safe::PaintJoin::Miter)
        .set_path_effect(effect);
    Ok(paint)
}
