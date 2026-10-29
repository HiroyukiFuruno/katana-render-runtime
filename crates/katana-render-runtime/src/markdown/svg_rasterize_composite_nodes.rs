use super::path;
use super::{AuthoredClipInfo, TextRasterizer, groups, layers, text};
use resvg::usvg;
use tiny_skia::Transform;

pub(super) fn render_node(
    node: &usvg::Node,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    authored_clip_info: &AuthoredClipInfo,
    text_rasterizer: Option<&dyn TextRasterizer>,
) -> Result<(), String> {
    match node {
        usvg::Node::Text(text) => {
            text::render_text_node(node, text, transform, pixmap, text_rasterizer)
        }
        usvg::Node::Group(group) => groups::render_group_node(
            node,
            group,
            transform,
            pixmap,
            authored_clip_info,
            text_rasterizer,
        ),
        usvg::Node::Path(path_node) if text_rasterizer.is_some() => {
            render_path_node(node, path_node, transform, pixmap)
        }
        _ => render_existing_path_node(node, transform, pixmap),
    }
}

fn render_path_node(
    node: &usvg::Node,
    path_node: &usvg::Path,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
) -> Result<(), String> {
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
