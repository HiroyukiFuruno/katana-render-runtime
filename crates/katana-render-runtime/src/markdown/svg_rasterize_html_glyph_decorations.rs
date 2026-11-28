use super::super::composite::path::{self, PreparedPath};
use resvg::usvg;
use skia_safe::{Canvas, Paint};

struct DecorationPath {
    path: PreparedPath,
    order: usvg::PaintOrder,
}

pub(super) struct Effects {
    before: Vec<DecorationPath>,
    after: Vec<DecorationPath>,
    pub(super) stroke: Option<Paint>,
    pub(super) order: usvg::PaintOrder,
}

pub(super) fn has_effects(span: &usvg::layout::Span) -> bool {
    span.stroke.is_some() || has_decorations(span)
}

pub(super) fn has_decorations(span: &usvg::layout::Span) -> bool {
    span.underline.is_some() || span.overline.is_some() || span.line_through.is_some()
}

pub(super) fn prepare(span: &usvg::layout::Span) -> Option<Effects> {
    let stroke = match &span.stroke {
        Some(stroke) if stroke.dasharray().is_some() => return None,
        stroke => path::style::stroke(stroke.as_ref(), usvg::ShapeRendering::GeometricPrecision)?,
    };
    let before = prepare_paths([span.overline.as_ref(), span.underline.as_ref()])?;
    let after = prepare_paths([span.line_through.as_ref()])?;
    Some(Effects {
        before,
        after,
        stroke,
        order: span.paint_order,
    })
}

fn prepare_paths<const N: usize>(paths: [Option<&usvg::Path>; N]) -> Option<Vec<DecorationPath>> {
    let mut result = Vec::new();
    /* WHY: usvgは装飾の可視性をspanから設定し、準備側は不可視spanを先に除外する。 */
    for node in paths.into_iter().flatten() {
        let prepared = path::prepare_path(node)?;
        result.push(DecorationPath {
            path: prepared,
            order: node.paint_order(),
        });
    }
    Some(result)
}

pub(super) fn draw_before(canvas: &Canvas, effects: &Effects, scale: f32) {
    draw_paths(canvas, &effects.before, scale);
}

pub(super) fn draw_after(canvas: &Canvas, effects: &Effects, scale: f32) {
    draw_paths(canvas, &effects.after, scale);
}

fn draw_paths(canvas: &Canvas, paths: &[DecorationPath], scale: f32) {
    let save = canvas.save();
    canvas.scale((scale, scale));
    for decoration in paths {
        path::draw_path(canvas, &decoration.path, decoration.order);
    }
    canvas.restore_to_count(save);
}
