use super::layers;
use resvg::usvg;
use tiny_skia::Transform;

pub(crate) trait TextRasterizer {
    fn render(
        &self,
        text: &usvg::Text,
        transform: Transform,
        pixmap: &mut tiny_skia::PixmapMut<'_>,
    ) -> Result<bool, String>;
}

pub(super) fn render_text_node(
    node: &usvg::Node,
    text: &usvg::Text,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    text_rasterizer: Option<&dyn TextRasterizer>,
) -> Result<(), String> {
    if let Some(text_rasterizer) = text_rasterizer
        && text_rasterizer.render(text, transform, pixmap)?
    {
        return Ok(());
    }
    layers::render_existing_node(node, transform, pixmap);
    Ok(())
}
