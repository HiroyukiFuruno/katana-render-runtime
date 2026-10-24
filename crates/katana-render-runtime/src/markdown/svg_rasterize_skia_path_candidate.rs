use resvg::usvg::{FillRule, Node, PaintOrder};
use skia_safe::{
    AlphaType, Canvas, ColorType, ImageInfo, Matrix, Paint, Path as SkiaPath, PathBuilder,
    PathFillType, surfaces,
};
use tiny_skia::{PathSegment, PixmapMut, Transform};

#[path = "svg_rasterize_skia_path_paint_candidate.rs"]
mod paint;

#[cfg(test)]
#[path = "svg_rasterize_skia_path_candidate_tests.rs"]
mod tests;

const MATRIX_PERSPECTIVE_ZERO: f32 = 0.0;
const MATRIX_PERSPECTIVE_ONE: f32 = 1.0;

pub(super) fn try_render(
    node: &Node,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
) -> Result<bool, String> {
    let Node::Path(path) = node else {
        return Ok(false);
    };
    if !path.is_visible() {
        return Ok(true);
    }
    validate_transform(transform)?;
    if !paint::supported_paints(path.fill(), path.stroke()) {
        return Ok(false);
    }
    let skia_path = build_path(path.data(), path.fill().map(|fill| fill.rule()))?;
    let fill = paint::make_fill(path.fill(), path.rendering_mode())?;
    let stroke = paint::make_stroke(path.stroke(), path.rendering_mode())?;
    if fill.is_none() && stroke.is_none() {
        return Ok(true);
    }
    render_on_surface(
        path.paint_order(),
        transform,
        pixmap,
        &skia_path,
        fill,
        stroke,
    )
}

fn render_on_surface(
    order: PaintOrder,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
    path: &SkiaPath,
    fill: Option<Paint>,
    stroke: Option<Paint>,
) -> Result<bool, String> {
    let info = ImageInfo::new(
        (pixmap.width() as i32, pixmap.height() as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut surface =
        surfaces::wrap_pixels(&info, pixmap.data_mut(), Some(info.min_row_bytes()), None)
            .ok_or_else(|| "failed to wrap the target pixmap for Skia".to_string())?;
    let canvas = surface.canvas();
    canvas.concat(&skia_matrix(transform));
    match order {
        PaintOrder::FillAndStroke => draw_pair(canvas, path, fill, stroke),
        PaintOrder::StrokeAndFill => draw_pair(canvas, path, stroke, fill),
    }
    Ok(true)
}

fn draw_pair(canvas: &Canvas, path: &SkiaPath, first: Option<Paint>, second: Option<Paint>) {
    if let Some(paint) = first {
        canvas.draw_path(path, &paint);
    }
    if let Some(paint) = second {
        canvas.draw_path(path, &paint);
    }
}

fn build_path(source: &tiny_skia::Path, fill_rule: Option<FillRule>) -> Result<SkiaPath, String> {
    let mut builder = PathBuilder::new();
    if matches!(fill_rule, Some(FillRule::EvenOdd)) {
        builder.set_fill_type(PathFillType::EvenOdd);
    }
    for segment in source.segments() {
        match segment {
            PathSegment::MoveTo(point) => {
                builder.move_to((point.x, point.y));
            }
            PathSegment::LineTo(point) => {
                builder.line_to((point.x, point.y));
            }
            PathSegment::QuadTo(control, point) => {
                builder.quad_to((control.x, control.y), (point.x, point.y));
            }
            PathSegment::CubicTo(first, second, point) => {
                builder.cubic_to((first.x, first.y), (second.x, second.y), (point.x, point.y));
            }
            PathSegment::Close => {
                builder.close();
            }
        }
    }
    let path = builder.detach();
    if !path.is_finite() {
        return Err("Skia path contains non-finite coordinates".to_string());
    }
    Ok(path)
}

fn validate_transform(transform: Transform) -> Result<(), String> {
    let values = [
        transform.sx,
        transform.ky,
        transform.kx,
        transform.sy,
        transform.tx,
        transform.ty,
    ];
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err("path transform contains non-finite values".to_string())
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
        MATRIX_PERSPECTIVE_ZERO,
        MATRIX_PERSPECTIVE_ZERO,
        MATRIX_PERSPECTIVE_ONE,
    )
}
