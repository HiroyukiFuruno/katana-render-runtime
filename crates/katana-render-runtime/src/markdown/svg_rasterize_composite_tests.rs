use super::{AuthoredClipInfo, SvgCompositeRenderer, TextRasterizer, layers};
use std::collections::HashSet;

#[path = "svg_rasterize_composite_text_hook_tests.rs"]
mod text_hook_tests;
#[path = "svg_rasterize_composite_transformed_clip_tests.rs"]
mod transformed_clip_tests;
pub(super) use text_hook_tests::{FailingTextRasterizer, TEXT_RASTERIZER_FAILURE};

const SVG_CANVAS_SIZE: u32 = 40;

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

fn assert_matches_existing_resvg(svg: &str) -> Result<(), String> {
    let expected = render_existing_svg(
        svg,
        SVG_CANVAS_SIZE,
        SVG_CANVAS_SIZE,
        tiny_skia::Transform::identity(),
    )?;
    let actual = render_svg(
        svg,
        SVG_CANVAS_SIZE,
        SVG_CANVAS_SIZE,
        tiny_skia::Transform::identity(),
    )?;
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
