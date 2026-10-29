use super::{AuthoredClipInfo, TextRasterizer, layers};
use resvg::usvg;
use tiny_skia::{PixmapMut, Transform};

#[path = "svg_rasterize_composite_opacity.rs"]
mod opacity;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_composite_groups_opacity_tests.rs"]
mod opacity_tests;

pub(super) fn render_group_node(
    node: &usvg::Node,
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
    provenance: &AuthoredClipInfo,
    text: Option<&dyn TextRasterizer>,
) -> Result<(), String> {
    if super::is_traversable_group(group) {
        super::render_traversable_group(node, group, transform, pixmap, provenance, text)
    } else {
        render_effect_group(node, group, transform, pixmap, provenance, text)
    }
}

fn render_effect_group(
    node: &usvg::Node,
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
    provenance: &AuthoredClipInfo,
    text: Option<&dyn TextRasterizer>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if render_linear_component_transfer(group, transform, pixmap, provenance, text)? {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    if text.is_some() && render_shadow_group(group, transform, pixmap, provenance, text)? {
        return Ok(());
    }
    if let Some(text_rasterizer) = text.filter(|_| opacity::supports(group)) {
        return opacity::render(group, transform, pixmap, provenance, text_rasterizer);
    }
    layers::render_existing_node(node, transform, pixmap);
    Ok(())
}

#[cfg(target_os = "macos")]
fn render_linear_component_transfer(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
    provenance: &AuthoredClipInfo,
    text: Option<&dyn TextRasterizer>,
) -> Result<bool, String> {
    let Some(text_rasterizer) = text else {
        return Ok(false);
    };
    let child_transform = transform.pre_concat(group.transform());
    super::linear_component_transfer::render(
        group,
        child_transform,
        pixmap,
        provenance,
        text_rasterizer,
    )
}

#[cfg(target_os = "macos")]
fn render_shadow_group(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
    provenance: &AuthoredClipInfo,
    text: Option<&dyn TextRasterizer>,
) -> Result<bool, String> {
    super::super::html_shadow::HtmlShadowRasterizer::render(
        group,
        transform.pre_concat(group.transform()),
        pixmap,
        &mut |local, target| {
            super::render_children_with_text(group, local, target, provenance, text)
        },
    )
}
