use super::super::provenance::AuthoredClipInfo;
use super::{TextRasterizer, arc_plan, contains_clip_ancestor_blend, render_root_children};
use resvg::usvg;
use tiny_skia::Transform;

pub(crate) struct SvgCompositeRenderer;
pub(super) struct RenderContext<'a, 'tree> {
    pub(super) provenance: &'a AuthoredClipInfo,
    pub(super) text: Option<&'a dyn TextRasterizer>,
    pub(super) plan: Option<&'a arc_plan::NativeArcPlan<'tree>>,
}
impl SvgCompositeRenderer {
    #[cfg(test)]
    pub(crate) fn render(
        tree: &usvg::Tree,
        transform: Transform,
        pixmap: &mut tiny_skia::PixmapMut<'_>,
        authored_clip_info: &AuthoredClipInfo,
    ) -> Result<(), String> {
        Self::render_with_text(tree, transform, pixmap, authored_clip_info, None)
    }
    pub(crate) fn render_with_text(
        tree: &usvg::Tree,
        transform: Transform,
        pixmap: &mut tiny_skia::PixmapMut<'_>,
        authored_clip_info: &AuthoredClipInfo,
        text_rasterizer: Option<&dyn TextRasterizer>,
    ) -> Result<(), String> {
        Self::render_with_plan(
            tree,
            transform,
            pixmap,
            authored_clip_info,
            text_rasterizer,
            None,
        )
    }
    pub(crate) fn render_full_with_text(
        tree: &usvg::Tree,
        transform: Transform,
        pixmap: &mut tiny_skia::PixmapMut<'_>,
        authored_clip_info: &AuthoredClipInfo,
        text_rasterizer: Option<&dyn TextRasterizer>,
    ) -> Result<(), String> {
        if text_rasterizer.is_none() || authored_clip_info.has_object_bounding_box {
            return Self::render_with_text(
                tree,
                transform,
                pixmap,
                authored_clip_info,
                text_rasterizer,
            );
        }
        let plan = arc_plan::NativeArcPlan::build(tree)?;
        Self::render_with_plan(
            tree,
            transform,
            pixmap,
            authored_clip_info,
            text_rasterizer,
            Some(&plan),
        )
    }
    fn render_with_plan<'tree>(
        tree: &'tree usvg::Tree,
        transform: Transform,
        pixmap: &mut tiny_skia::PixmapMut<'_>,
        authored_clip_info: &AuthoredClipInfo,
        text_rasterizer: Option<&dyn TextRasterizer>,
        plan: Option<&arc_plan::NativeArcPlan<'tree>>,
    ) -> Result<(), String> {
        #[cfg(all(test, target_os = "macos"))]
        super::super::SvgRasterizeOps::record_render_dispatch(
            text_rasterizer.is_some(),
            authored_clip_info.has_object_bounding_box,
        );
        if authored_clip_info.has_object_bounding_box {
            resvg::render(tree, transform, pixmap);
            return Ok(());
        }
        let context = RenderContext {
            provenance: authored_clip_info,
            text: text_rasterizer,
            plan,
        };
        render_root_children(tree, transform, pixmap, &context)
    }
    pub(crate) fn needs_authored_clip_ids(tree: &usvg::Tree) -> bool {
        contains_clip_ancestor_blend(tree.root(), false, &AuthoredClipInfo::default())
    }
}
