use super::provenance::AuthoredClipInfo;
use resvg::usvg;
use tiny_skia::Transform;
#[path = "svg_rasterize_composite_arc_plan.rs"]
mod arc_plan;
#[path = "svg_rasterize_composite_context.rs"]
mod context;
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
#[path = "svg_rasterize_composite_path.rs"]
pub(super) mod path;
#[path = "svg_rasterize_composite_text.rs"]
mod text;
use context::RenderContext;
pub(crate) use context::SvgCompositeRenderer;
pub(crate) use text::TextRasterizer;
fn render_root_children(
    tree: &usvg::Tree,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    context: &RenderContext<'_, '_>,
) -> Result<(), String> {
    let needs_authored_clip_ids = SvgCompositeRenderer::needs_authored_clip_ids(tree);
    match context.text {
        Some(_) => render_children_with_context(
            tree.root(),
            transform.pre_concat(tree.root().transform()),
            pixmap,
            context,
        ),
        None if needs_authored_clip_ids => {
            render_children_with_context(tree.root(), transform, pixmap, context)
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

fn render_children_with_text(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    authored_clip_info: &AuthoredClipInfo,
    text_rasterizer: Option<&dyn TextRasterizer>,
) -> Result<(), String> {
    let context = RenderContext {
        provenance: authored_clip_info,
        text: text_rasterizer,
        plan: None,
    };
    render_children_with_context(group, transform, pixmap, &context)
}

fn render_children_with_context(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    context: &RenderContext<'_, '_>,
) -> Result<(), String> {
    for child in group.children() {
        nodes::render_node(child, transform, pixmap, context)?;
    }
    Ok(())
}

fn render_traversable_group(
    node: &usvg::Node,
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    context: &RenderContext<'_, '_>,
) -> Result<(), String> {
    let child_transform = transform.pre_concat(group.transform());
    if generated_viewport_clip(group, context.provenance) {
        return layers::render_clipped_group_with_context(group, child_transform, pixmap, context);
    }
    if group.clip_path().is_none() {
        return render_children_with_context(group, child_transform, pixmap, context);
    }
    layers::render_existing_node(node, transform, pixmap);
    Ok(())
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
#[path = "svg_rasterize_composite_arc_integration_tests.rs"]
mod arc_integration_tests;

#[cfg(test)]
#[path = "svg_rasterize_composite_mask_tests.rs"]
mod mask_tests;

#[cfg(test)]
mod tests {
    use super::{AuthoredClipInfo, SvgCompositeRenderer, TextRasterizer, layers};
    use std::cell::Cell;
    use std::collections::HashSet;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><clipPath id="window"><rect x="10" y="10" width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g clip-path="url(#window)"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg>"##;
    const VIEWPORT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40" fill="#202020"/><svg x="10" y="10" width="20" height="20" transform="translate(3 2) rotate(5 20 20)"><rect width="20" height="20" fill="#e0e0e0" style="mix-blend-mode:multiply"/></svg></svg>"##;
    const VIEWPORT_OVERFLOW_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40" fill="#202020"/><svg x="10" y="10" width="20" height="20"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></svg></svg>"##;
    const VIEWPORT_NORMAL_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40" fill="#202020"/><svg x="10" y="10" width="20" height="20"><rect width="20" height="20" fill="#e0e0e0"/></svg></svg>"##;
    const VIEWPORT_EXPLICIT_CLIP_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><clipPath id="window"><rect width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><svg x="10" y="10" width="20" height="20"><g clip-path="url(#window)"><rect width="20" height="20" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg></svg>"##;
    const OBJECT_BOUNDING_BOX_CLIP_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><clipPath id="window" clipPathUnits="objectBoundingBox"><rect width="1" height="1"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g clip-path="url(#window)"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg>"##;
    const REUSED_OBJECT_BOUNDING_BOX_CLIP_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><clipPath id="window" clipPathUnits="objectBoundingBox"><rect width="1" height="1"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g clip-path="url(#window)"><rect width="20" height="20" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g><g transform="translate(20 20)" clip-path="url(#window)"><rect width="20" height="20" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg>"##;
    const NESTED_CLIP_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><clipPath id="inner"><rect x="14" y="14" width="12" height="12"/></clipPath><clipPath id="window"><rect x="10" y="10" width="20" height="20" clip-path="url(#inner)"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g transform="translate(3 2) rotate(5 20 20)" clip-path="url(#window)"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg>"##;
    const HALF_ALPHA_BACKDROP_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40" fill="#202020" fill-opacity="0.5"/><svg x="10" y="10" width="20" height="20" transform="translate(3 2) rotate(5 20 20)"><rect width="20" height="20" fill="#e0e0e0" style="mix-blend-mode:multiply"/></svg></svg>"##;
    const HALF_ALPHA_UNCLIPPED_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><rect width="40" height="40" fill="#202020" fill-opacity="0.5"/><rect x="10" y="10" width="20" height="20" fill="#e0e0e0" style="mix-blend-mode:multiply"/></svg>"##;
    const SOURCE_OVER_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><clipPath id="window"><rect x="10" y="10" width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g clip-path="url(#window)"><rect width="40" height="40" fill="#e0e0e0"/></g></svg>"##;
    const ISOLATED_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><clipPath id="window"><rect x="10" y="10" width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g clip-path="url(#window)" style="isolation:isolate"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg>"##;
    const OPACITY_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><clipPath id="window"><rect x="10" y="10" width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g opacity="0.5" clip-path="url(#window)"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg>"##;
    const FILTER_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><filter id="blur"><feGaussianBlur stdDeviation="0"/></filter><clipPath id="window"><rect x="10" y="10" width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g filter="url(#blur)" clip-path="url(#window)"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg>"##;
    const MASK_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><mask id="window"><rect width="40" height="40" fill="white"/></mask><clipPath id="clip"><rect x="10" y="10" width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g mask="url(#window)" clip-path="url(#clip)"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g></svg>"##;
    const TEXT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><text x="2" y="20" font-size="12">text</text></svg>"##;
    const TEXT_VIEWPORT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><svg x="5" y="5" width="30" height="30"><text x="2" y="20" font-size="12">text</text></svg></svg>"##;
    const ROOT_TRANSFORM_TEXT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40" transform="translate(3 4)"><text x="2" y="20" font-size="12">text</text></svg>"##;
    const FILTER_TEXT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><filter id="blur"><feGaussianBlur stdDeviation="0"/></filter></defs><g filter="url(#blur)"><text x="2" y="20" font-size="12">text</text></g></svg>"##;

    pub(super) struct FallbackTextRasterizer;

    impl TextRasterizer for FallbackTextRasterizer {
        fn render(
            &self,
            _text: &resvg::usvg::Text,
            _transform: tiny_skia::Transform,
            _pixmap: &mut tiny_skia::PixmapMut<'_>,
        ) -> Result<bool, String> {
            Ok(false)
        }
    }

    struct CountingTextRasterizer {
        calls: Cell<usize>,
        first_transform: Cell<Option<(f32, f32)>>,
    }

    struct TextHookObservation {
        calls: usize,
        first_transform: Option<(f32, f32)>,
    }

    impl TextRasterizer for CountingTextRasterizer {
        fn render(
            &self,
            _text: &resvg::usvg::Text,
            transform: tiny_skia::Transform,
            _pixmap: &mut tiny_skia::PixmapMut<'_>,
        ) -> Result<bool, String> {
            self.calls.set(self.calls.get() + 1);
            self.first_transform.set(Some((transform.tx, transform.ty)));
            Ok(true)
        }
    }

    fn observe_text_hook(svg: &str) -> Result<TextHookObservation, String> {
        let (tree, mut pixmap) = parse_svg_pixmap_with_bundled_fonts(svg, 40, 40)?;
        let rasterizer = CountingTextRasterizer {
            calls: Cell::new(0),
            first_transform: Cell::new(None),
        };
        SvgCompositeRenderer::render_with_text(
            &tree,
            tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
            &AuthoredClipInfo::default(),
            Some(&rasterizer),
        )?;
        Ok(TextHookObservation {
            calls: rasterizer.calls.get(),
            first_transform: rasterizer.first_transform.get(),
        })
    }

    #[test]
    fn clipped_nonisolating_blend_uses_the_existing_backdrop() -> Result<(), String> {
        let pixmap = render_svg(SVG, 40, 40, tiny_skia::Transform::identity())?;
        let pixels = pixmap.data();
        let offset = (15 * 40 + 15) * 4;
        assert_eq!(&pixels[offset..offset + 4], &[224, 224, 224, 255]);
        let outside = (5 * 40 + 5) * 4;
        assert_eq!(&pixels[outside..outside + 4], &[32, 32, 32, 255]);
        Ok(())
    }

    #[test]
    fn generated_viewport_clip_blends_against_the_existing_backdrop() -> Result<(), String> {
        let pixmap = render_svg(VIEWPORT_SVG, 40, 40, tiny_skia::Transform::identity())?;
        let offset = (15 * 40 + 15) * 4;
        assert_eq!(&pixmap.data()[offset..offset + 4], &[28, 28, 28, 255]);
        let outside = (2 * 40 + 2) * 4;
        assert_eq!(&pixmap.data()[outside..outside + 4], &[32, 32, 32, 255]);
        Ok(())
    }

    #[test]
    fn generated_viewport_clip_limits_overflowing_blend_children() -> Result<(), String> {
        let pixmap = render_svg(
            VIEWPORT_OVERFLOW_SVG,
            40,
            40,
            tiny_skia::Transform::identity(),
        )?;
        let inside = (15 * 40 + 15) * 4;
        assert_eq!(&pixmap.data()[inside..inside + 4], &[28, 28, 28, 255]);
        let outside = (15 * 40 + 35) * 4;
        assert_eq!(&pixmap.data()[outside..outside + 4], &[32, 32, 32, 255]);
        Ok(())
    }

    #[test]
    fn generated_viewport_crop_is_bounded_inside_the_canvas() -> Result<(), String> {
        let (tree, mut pixmap) = parse_svg_pixmap(VIEWPORT_SVG, 40, 40)?;
        let group = tree
            .root()
            .children()
            .iter()
            .find_map(|node| match node {
                resvg::usvg::Node::Group(group) if group.clip_path().is_some() => Some(group),
                _ => None,
            })
            .ok_or_else(|| "generated viewport group missing".to_string())?;
        let crop =
            layers::clipped_group_rect(group, tiny_skia::Transform::identity(), &pixmap.as_mut())?
                .ok_or_else(|| "generated viewport crop missing".to_string())?;
        assert!(crop.width() < pixmap.width());
        assert!(crop.height() < pixmap.height());
        Ok(())
    }

    #[test]
    fn generated_viewport_without_blend_matches_existing_resvg_boundary() -> Result<(), String> {
        assert_matches_existing_resvg(VIEWPORT_NORMAL_SVG)
    }

    #[test]
    fn generated_viewport_with_explicit_clip_keeps_authored_clip_isolated() -> Result<(), String> {
        let pixmap = render_svg(
            VIEWPORT_EXPLICIT_CLIP_SVG,
            40,
            40,
            tiny_skia::Transform::identity(),
        )?;
        let offset = (15 * 40 + 15) * 4;
        assert_eq!(&pixmap.data()[offset..offset + 4], &[224, 224, 224, 255]);
        Ok(())
    }

    #[test]
    fn authored_object_bounding_box_clip_stays_on_existing_resvg_path() -> Result<(), String> {
        assert_matches_existing_resvg(OBJECT_BOUNDING_BOX_CLIP_SVG)
    }

    #[test]
    fn reused_object_bounding_box_clip_stays_on_existing_resvg_path() -> Result<(), String> {
        assert_matches_existing_resvg(REUSED_OBJECT_BOUNDING_BOX_CLIP_SVG)
    }

    #[test]
    fn generated_viewport_blend_preserves_parent_transform_and_dpr() -> Result<(), String> {
        for (scale, width, height) in [(1.0, 40, 40), (2.0, 80, 80)] {
            let pixmap = render_svg(
                VIEWPORT_SVG,
                width,
                height,
                tiny_skia::Transform::from_scale(scale, scale),
            )?;
            assert!(
                pixmap
                    .data()
                    .chunks_exact(4)
                    .any(|pixel| pixel == [28, 28, 28, 255]),
                "nested clip blend missing at scale {scale}"
            );
            assert!(
                pixmap
                    .data()
                    .chunks_exact(4)
                    .any(|pixel| { pixel[3] == 255 && (29..32).contains(&pixel[0]) }),
                "fractional opaque clip coverage missing at scale {scale}"
            );
        }
        Ok(())
    }

    #[test]
    fn authored_nested_clip_stays_on_existing_resvg_path_at_each_dpr() -> Result<(), String> {
        for (scale, width, height) in [(1.0, 40, 40), (2.0, 80, 80)] {
            let actual = render_svg(
                NESTED_CLIP_SVG,
                width,
                height,
                tiny_skia::Transform::from_scale(scale, scale),
            )?;
            let expected = render_existing_svg(
                NESTED_CLIP_SVG,
                width,
                height,
                tiny_skia::Transform::from_scale(scale, scale),
            )?;
            assert_eq!(actual.data(), expected.data());
        }
        Ok(())
    }

    #[test]
    fn clipped_blend_preserves_alpha_backdrop_without_double_compositing() -> Result<(), String> {
        let clipped = render_svg(
            HALF_ALPHA_BACKDROP_SVG,
            40,
            40,
            tiny_skia::Transform::identity(),
        )?;
        let unclipped = render_svg(
            HALF_ALPHA_UNCLIPPED_SVG,
            40,
            40,
            tiny_skia::Transform::identity(),
        )?;
        let offset = (22 * 40 + 23) * 4;
        assert_eq!(
            &clipped.data()[offset..offset + 4],
            &unclipped.data()[offset..offset + 4]
        );
        assert_fractional_alpha(&clipped);
        let outside = (5 * 40 + 5) * 4;
        assert_eq!(&clipped.data()[outside..outside + 4], &[16, 16, 16, 128]);
        Ok(())
    }

    fn assert_fractional_alpha(pixmap: &tiny_skia::Pixmap) {
        assert!(
            pixmap
                .data()
                .chunks_exact(4)
                .any(|pixel| { (129..255).contains(&pixel[3]) && (17..126).contains(&pixel[0]) })
        );
    }

    #[test]
    fn explicit_isolation_and_source_over_stay_on_existing_resvg_path() -> Result<(), String> {
        for svg in [SOURCE_OVER_SVG, ISOLATED_SVG] {
            assert_matches_existing_resvg(svg)?;
        }
        Ok(())
    }

    #[test]
    fn isolating_group_features_stay_on_existing_resvg_path() -> Result<(), String> {
        for svg in [OPACITY_SVG, FILTER_SVG, MASK_SVG] {
            assert_matches_existing_resvg(svg)?;
        }
        Ok(())
    }

    #[test]
    fn text_rasterizer_false_preserves_existing_resvg_pixels() -> Result<(), String> {
        let (tree, mut actual) = parse_svg_pixmap_with_bundled_fonts(TEXT_SVG, 40, 40)?;
        let expected = render_existing_svg_with_bundled_fonts(
            TEXT_SVG,
            40,
            40,
            tiny_skia::Transform::identity(),
        )?;
        SvgCompositeRenderer::render_with_text(
            &tree,
            tiny_skia::Transform::identity(),
            &mut actual.as_mut(),
            &AuthoredClipInfo::default(),
            Some(&FallbackTextRasterizer),
        )?;
        assert!(expected.data().chunks_exact(4).any(|pixel| pixel[3] != 0));
        assert_eq!(actual.data(), expected.data());
        Ok(())
    }

    #[test]
    fn text_rasterizer_receives_generated_viewport_children() -> Result<(), String> {
        let observation = observe_text_hook(TEXT_VIEWPORT_SVG)?;
        assert_eq!(observation.calls, 1);
        Ok(())
    }

    #[test]
    fn text_rasterizer_applies_root_transform_once() -> Result<(), String> {
        let observation = observe_text_hook(ROOT_TRANSFORM_TEXT_SVG)?;
        assert_eq!(observation.calls, 1);
        assert_eq!(observation.first_transform, Some((3.0, 4.0)));
        Ok(())
    }

    #[test]
    fn text_rasterizer_skips_unsupported_filter_groups() -> Result<(), String> {
        let (tree, mut actual) = parse_svg_pixmap_with_bundled_fonts(FILTER_TEXT_SVG, 40, 40)?;
        let expected = render_existing_svg_with_bundled_fonts(
            FILTER_TEXT_SVG,
            40,
            40,
            tiny_skia::Transform::identity(),
        )?;
        let rasterizer = CountingTextRasterizer {
            calls: Cell::new(0),
            first_transform: Cell::new(None),
        };
        SvgCompositeRenderer::render_with_text(
            &tree,
            tiny_skia::Transform::identity(),
            &mut actual.as_mut(),
            &AuthoredClipInfo::default(),
            Some(&rasterizer),
        )?;
        assert_eq!(rasterizer.calls.get(), 0);
        assert_eq!(actual.data(), expected.data());
        Ok(())
    }

    fn assert_matches_existing_resvg(svg: &str) -> Result<(), String> {
        let expected = render_existing_svg(svg, 40, 40, tiny_skia::Transform::identity())?;
        let actual = render_svg(svg, 40, 40, tiny_skia::Transform::identity())?;
        assert_eq!(actual.data(), expected.data());
        Ok(())
    }

    pub(super) fn render_svg(
        svg: &str,
        width: u32,
        height: u32,
        transform: tiny_skia::Transform,
    ) -> Result<tiny_skia::Pixmap, String> {
        let (tree, mut pixmap) = parse_svg_pixmap(svg, width, height)?;
        let document =
            resvg::usvg::roxmltree::Document::parse(svg).map_err(|error| error.to_string())?;
        let clip_nodes = document
            .descendants()
            .filter(|node| node.has_tag_name("clipPath"))
            .collect::<Vec<_>>();
        let authored_clip_info = super::AuthoredClipInfo {
            ids: clip_nodes
                .iter()
                .filter_map(|node| node.attribute("id"))
                .map(str::to_owned)
                .collect::<HashSet<_>>(),
            generated_ids: HashSet::new(),
            has_object_bounding_box: clip_nodes
                .iter()
                .any(|node| node.attribute("clipPathUnits") == Some("objectBoundingBox")),
        };
        SvgCompositeRenderer::render(&tree, transform, &mut pixmap.as_mut(), &authored_clip_info)?;
        Ok(pixmap)
    }

    fn render_existing_svg(
        svg: &str,
        width: u32,
        height: u32,
        transform: tiny_skia::Transform,
    ) -> Result<tiny_skia::Pixmap, String> {
        let (tree, mut pixmap) = parse_svg_pixmap(svg, width, height)?;
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        Ok(pixmap)
    }

    fn render_existing_svg_with_bundled_fonts(
        svg: &str,
        width: u32,
        height: u32,
        transform: tiny_skia::Transform,
    ) -> Result<tiny_skia::Pixmap, String> {
        let (tree, mut pixmap) = parse_svg_pixmap_with_bundled_fonts(svg, width, height)?;
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        Ok(pixmap)
    }

    fn parse_svg_pixmap(
        svg: &str,
        width: u32,
        height: u32,
    ) -> Result<(resvg::usvg::Tree, tiny_skia::Pixmap), String> {
        let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
            .map_err(|error| error.to_string())?;
        let pixmap = tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| "failed to allocate expected SVG pixmap".to_string())?;
        Ok((tree, pixmap))
    }

    pub(super) fn parse_svg_pixmap_with_bundled_fonts(
        svg: &str,
        width: u32,
        height: u32,
    ) -> Result<(resvg::usvg::Tree, tiny_skia::Pixmap), String> {
        let options = super::super::rasterizer_options();
        let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(|error| error.to_string())?;
        let pixmap = tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| "failed to allocate expected SVG pixmap".to_string())?;
        Ok((tree, pixmap))
    }
}
