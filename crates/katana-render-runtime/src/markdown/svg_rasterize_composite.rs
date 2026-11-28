use super::provenance::AuthoredClipInfo;
use resvg::usvg;
use tiny_skia::Transform;
#[path = "svg_rasterize_composite_groups.rs"]
mod groups;
#[path = "svg_rasterize_composite_layers.rs"]
mod layers;
#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_linear_component_transfer.rs"]
mod linear_component_transfer;
#[path = "svg_rasterize_composite_mask.rs"]
mod mask;
#[path = "svg_rasterize_composite_nodes.rs"]
mod nodes;
use nodes::{render_children_at_origin, render_children_with_text};
#[path = "svg_rasterize_composite_path.rs"]
pub(super) mod path;
#[path = "svg_rasterize_composite_text.rs"]
mod text;
pub(crate) use text::TextRasterizer;
pub(crate) struct SvgCompositeRenderer;

#[derive(Clone, Copy)]
struct TileContext {
    origin: (i32, i32),
    frame: (i32, u32),
}

fn pixmap_width(pixmap: &tiny_skia::PixmapMut<'_>) -> i32 {
    /* WHY: tiny-skiaは生成時にRGBAの行バイト数がi32に収まる幅だけを受け入れる。 */
    pixmap.width() as i32
}

impl TileContext {
    fn cropped(self, crop: tiny_skia::IntRect) -> Result<Self, String> {
        let x = self
            .origin
            .0
            .checked_add(crop.x())
            .ok_or("SVG crop origin overflow")?;
        let y = self
            .origin
            .1
            .checked_add(crop.y())
            .ok_or("SVG crop origin overflow")?;
        Ok(Self {
            origin: (x, y),
            ..self
        })
    }
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
        #[cfg(all(test, target_os = "macos"))]
        super::SvgRasterizeOps::record_render_dispatch(
            text_rasterizer.is_some(),
            authored_clip_info.has_object_bounding_box,
        );
        if authored_clip_info.has_object_bounding_box {
            resvg::render(tree, transform, pixmap);
            return Ok(());
        }
        render_root_children(tree, transform, pixmap, authored_clip_info, text_rasterizer)
    }
    pub(crate) fn needs_authored_clip_ids(tree: &usvg::Tree) -> bool {
        contains_clip_ancestor_blend(tree.root(), false, &AuthoredClipInfo::default())
    }
}

fn render_root_children(
    tree: &usvg::Tree,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    authored_clip_info: &AuthoredClipInfo,
    text_rasterizer: Option<&dyn TextRasterizer>,
) -> Result<(), String> {
    let needs_authored_clip_ids = SvgCompositeRenderer::needs_authored_clip_ids(tree);
    let origin = Some(TileContext {
        origin: (0, 0),
        frame: (pixmap_width(pixmap), pixmap.height()),
    });
    match text_rasterizer {
        Some(text_rasterizer) => render_children_at_origin(
            tree.root(),
            transform.pre_concat(tree.root().transform()),
            pixmap,
            authored_clip_info,
            Some(text_rasterizer),
            origin,
        ),
        None if needs_authored_clip_ids => {
            render_children_with_text(tree.root(), transform, pixmap, authored_clip_info, None)
        }
        None => {
            resvg::render(tree, transform, pixmap);
            Ok(())
        }
    }
}
fn contains_clip_ancestor_blend(
    group: &usvg::Group,
    clip_ancestor: bool,
    authored_clip_info: &AuthoredClipInfo,
) -> bool {
    for node in group.children() {
        let usvg::Node::Group(child) = node else {
            continue;
        };
        if !is_traversable_group(child) {
            if has_nonisolating_blend(child, clip_ancestor) {
                return true;
            }
            continue;
        }
        let generated_clip = generated_viewport_clip(child, authored_clip_info);
        if child.clip_path().is_some() && !generated_clip {
            continue;
        }
        let has_clip = clip_ancestor || generated_clip;
        if contains_clip_ancestor_blend(child, has_clip, authored_clip_info) {
            return true;
        }
    }
    false
}
fn is_traversable_group(group: &usvg::Group) -> bool {
    group.opacity().get() == 1.0
        && group.blend_mode() == usvg::BlendMode::Normal
        && !group.isolate()
        && group.filters().is_empty()
        && group.mask().is_none()
}
fn has_nonisolating_blend(group: &usvg::Group, clip_ancestor: bool) -> bool {
    clip_ancestor
        && group.opacity().get() == 1.0
        && group.blend_mode() != usvg::BlendMode::Normal
        && !group.isolate()
        && group.filters().is_empty()
        && group.mask().is_none()
        && group.clip_path().is_none()
}
fn generated_viewport_clip(group: &usvg::Group, authored_clip_info: &AuthoredClipInfo) -> bool {
    group
        .clip_path()
        .is_some_and(|clip| is_generated_viewport_clip(clip, authored_clip_info))
}

fn render_traversable_group(
    node: &usvg::Node,
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    authored_clip_info: &AuthoredClipInfo,
    text_rasterizer: Option<&dyn TextRasterizer>,
    origin: Option<TileContext>,
) -> Result<(), String> {
    let child_transform = transform.pre_concat(group.transform());
    let clip = group.clip_path();
    if clip.is_some_and(|clip| !is_generated_viewport_clip(clip, authored_clip_info)) {
        layers::render_existing_node(node, transform, pixmap);
        return Ok(());
    }
    layers::render_group_with_viewport_clip(
        group,
        (clip, child_transform),
        pixmap,
        authored_clip_info,
        text_rasterizer,
        origin,
    )
}

fn is_generated_viewport_clip(
    clip: &usvg::ClipPath,
    authored_clip_info: &AuthoredClipInfo,
) -> bool {
    authored_clip_info.generated_ids.contains(clip.id())
        || (!authored_clip_info.ids.contains(clip.id()) && clip.transform().is_identity())
}

#[cfg(test)]
#[path = "svg_rasterize_composite_stroke_bounds_tests.rs"]
mod stroke_bounds_tests;

#[cfg(test)]
#[path = "svg_rasterize_composite_mask_tests.rs"]
mod mask_tests;

#[cfg(test)]
#[path = "svg_rasterize_composite_tests.rs"]
mod tests;
