use super::{AuthoredClipInfo, TextRasterizer};
use resvg::usvg;

#[derive(Clone, Copy)]
pub(super) struct RenderContext<'a> {
    pub(super) authored_clip_info: &'a AuthoredClipInfo,
    pub(super) text_rasterizer: Option<&'a dyn TextRasterizer>,
    pub(super) primitive_tree: Option<&'a usvg::Tree>,
}

impl<'a> RenderContext<'a> {
    pub(super) fn strict(
        authored_clip_info: &'a AuthoredClipInfo,
        text_rasterizer: Option<&'a dyn TextRasterizer>,
    ) -> Self {
        Self {
            authored_clip_info,
            text_rasterizer,
            primitive_tree: None,
        }
    }

    pub(super) fn with_primitives(
        authored_clip_info: &'a AuthoredClipInfo,
        text_rasterizer: &'a dyn TextRasterizer,
        tree: &'a usvg::Tree,
    ) -> Self {
        Self {
            authored_clip_info,
            text_rasterizer: Some(text_rasterizer),
            primitive_tree: Some(tree),
        }
    }
}
