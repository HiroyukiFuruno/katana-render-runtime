use super::path;
use super::{RenderContext, groups, layers, text};
use resvg::usvg;
use tiny_skia::Transform;

pub(super) fn render_node(
    node: &usvg::Node,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    context: &RenderContext<'_, '_>,
) -> Result<(), String> {
    match node {
        usvg::Node::Text(text) => {
            text::render_text_node(node, text, transform, pixmap, context.text)
        }
        usvg::Node::Group(group) => {
            groups::render_group_node(node, group, transform, pixmap, context)
        }
        usvg::Node::Path(path_node) if context.text.is_some() => {
            render_path_node(node, path_node, transform, pixmap, context)
        }
        _ => render_existing_path_node(node, transform, pixmap),
    }
}

fn render_path_node(
    node: &usvg::Node,
    path_node: &usvg::Path,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    context: &RenderContext<'_, '_>,
) -> Result<(), String> {
    if let Some(prepared) = context.plan.and_then(|plan| plan.path(path_node))
        && path::transform_is_finite(transform)
    {
        path::render_prepared_path(pixmap, prepared, transform, path_node.paint_order())?;
        #[cfg(all(test, target_os = "macos"))]
        super::super::SvgRasterizeOps::record_path_route(true);
        return Ok(());
    }
    if path::render_if_supported(path_node, transform, pixmap)? {
        #[cfg(all(test, target_os = "macos"))]
        super::super::SvgRasterizeOps::record_path_route(true);
        Ok(())
    } else {
        #[cfg(all(test, target_os = "macos"))]
        super::super::SvgRasterizeOps::record_path_route(false);
        render_existing_path_node(node, transform, pixmap)
    }
}

fn render_existing_path_node(
    node: &usvg::Node,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
) -> Result<(), String> {
    layers::render_existing_node(node, transform, pixmap);
    Ok(())
}
