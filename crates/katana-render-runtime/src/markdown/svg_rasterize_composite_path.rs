use resvg::usvg;
use skia_safe::{AlphaType, ColorType, ImageInfo, M44, Matrix, Paint, Path};
use tiny_skia::{PixmapMut, Transform};

#[path = "svg_rasterize_composite_path_geometry.rs"]
mod geometry;
#[path = "svg_rasterize_composite_path_style.rs"]
pub(in crate::markdown::svg_rasterize) mod style;
#[path = "svg_rasterize_composite_path_tile.rs"]
mod tile;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_composite_path_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_composite_path_arc_probe_tests.rs"]
mod arc_probe_tests;

#[cfg(test)]
pub(super) fn render_if_supported(
    node: &usvg::Path,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
) -> Result<bool, String> {
    render_if_supported_at_origin(node, transform, pixmap, None)
}

pub(super) fn render_if_supported_at_origin(
    node: &usvg::Path,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
    origin: Option<super::TileContext>,
) -> Result<bool, String> {
    if !node.is_visible() {
        return Ok(true);
    }
    if !transform_is_finite(transform) {
        return Ok(false);
    }
    let Some(prepared) = prepare_path(node)? else {
        return Ok(false);
    };
    #[cfg(all(test, target_os = "macos"))]
    super::super::SvgRasterizeOps::record_path_paint(
        node,
        transform,
        (pixmap.width(), pixmap.height()),
        node.paint_order(),
        prepared.fill.as_ref(),
        prepared.stroke.as_ref(),
    );
    render_selected(node, pixmap, &prepared, transform, origin)?;
    Ok(true)
}

fn render_selected(
    node: &usvg::Path,
    pixmap: &mut PixmapMut<'_>,
    prepared: &PreparedPath,
    transform: Transform,
    origin: Option<super::TileContext>,
) -> Result<(), String> {
    if let Some(origin) = origin.filter(|_| tile::has_curve(node)) {
        tile::render(pixmap, node, prepared, transform, origin)
    } else {
        render_prepared_path(pixmap, prepared, transform, node.paint_order())
    }
}

fn render_prepared_path(
    pixmap: &mut PixmapMut<'_>,
    prepared: &PreparedPath,
    transform: Transform,
    order: usvg::PaintOrder,
) -> Result<(), String> {
    let dimensions = (pixmap.width() as i32, pixmap.height() as i32);
    let info = ImageInfo::new(dimensions, ColorType::RGBA8888, AlphaType::Premul, None);
    let mut surface =
        skia_safe::surfaces::wrap_pixels(&info, pixmap.data_mut(), info.min_row_bytes(), None)
            .ok_or_else(|| "Skia SVG path surface creation failed".to_owned())?;
    let matrix = skia_matrix(transform);
    surface.canvas().set_matrix(&M44::from(&matrix));
    draw_path(surface.canvas(), prepared, order);
    Ok(())
}

fn transform_is_finite(transform: Transform) -> bool {
    [
        transform.sx,
        transform.kx,
        transform.tx,
        transform.ky,
        transform.sy,
        transform.ty,
    ]
    .into_iter()
    .all(f32::is_finite)
}

pub(in crate::markdown::svg_rasterize) struct PreparedPath {
    path: Path,
    fill: Option<Paint>,
    stroke: Option<Paint>,
}

pub(in crate::markdown::svg_rasterize) fn prepare_path(
    node: &usvg::Path,
) -> Result<Option<PreparedPath>, String> {
    let Some(fill) = style::fill(node.fill(), node.rendering_mode()) else {
        return Ok(None);
    };
    let Some(stroke) = style::stroke(node.stroke(), node.rendering_mode()) else {
        return Ok(None);
    };
    if fill.is_none() && stroke.is_none() {
        return Ok(None);
    }
    let path = geometry::convert(node.data(), node.fill().map(|value| value.rule()))?;
    Ok(Some(PreparedPath { path, fill, stroke }))
}

pub(in crate::markdown::svg_rasterize) fn draw_path(
    canvas: &skia_safe::Canvas,
    prepared: &PreparedPath,
    order: usvg::PaintOrder,
) {
    let (first, second) = match order {
        usvg::PaintOrder::FillAndStroke => (prepared.fill.as_ref(), prepared.stroke.as_ref()),
        usvg::PaintOrder::StrokeAndFill => (prepared.stroke.as_ref(), prepared.fill.as_ref()),
    };
    for paint in [first, second].into_iter().flatten() {
        canvas.draw_path(&prepared.path, paint);
    }
}

fn skia_matrix(transform: Transform) -> Matrix {
    Matrix::new_all(
        transform.sx,
        transform.kx,
        transform.tx,
        transform.ky,
        transform.sy,
        transform.ty,
        0.0,
        0.0,
        1.0,
    )
}

fn solid_color(color: usvg::Color) -> Paint {
    let mut paint = Paint::default();
    paint.set_color(skia_safe::Color::from_rgb(
        color.red,
        color.green,
        color.blue,
    ));
    paint
}
