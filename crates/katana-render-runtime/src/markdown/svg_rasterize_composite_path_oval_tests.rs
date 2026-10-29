use super::super::super::html_glyph::HtmlGlyphRasterizer;
use super::super::{AuthoredClipInfo, RenderContext, SvgCompositeRenderer};
use super::{
    geometry, prepare_path, prepare_source_oval, render_if_supported,
    render_if_supported_with_tree, render_prepared_path,
};
use resvg::usvg;
use std::collections::HashSet;
use tiny_skia::{Pixmap, Transform};

#[path = "svg_rasterize_composite_path_oval_guard_tests.rs"]
mod guard_tests;

const SIZE: u32 = 64;
const OVAL_CX: f32 = 16.0;
const OVAL_CY: f32 = 20.0;
const OVAL_RX: f32 = 8.0;
const OVAL_RY: f32 = 6.0;
const ZERO: f32 = 0.0;
const ONE: f32 = 1.0;
const NONUNIFORM_SCALE_X: f32 = 2.0;
const NONUNIFORM_SCALE_Y: f32 = 0.5;
const REFLECTION_SCALE_X: f32 = -1.0;
const REFLECTION_TRANSLATE_X: f32 = 40.0;
const EDGE_TRANSLATE_X: f32 = -8.0;
const SKEW_X: f32 = 0.25;
const OUT_OF_BOUNDS_TRANSLATE_X: f32 = 65.0;
const ORDER_SAMPLE: (u32, u32) = (9, 20);
const CENTER_SAMPLE: (u32, u32) = (20, 20);
const OVAL_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><ellipse cx="16" cy="20" rx="8" ry="6" fill="#e02020" stroke="#2040e0" stroke-width="2"/></svg>"##;

#[test]
fn source_ellipse_uses_the_native_oval_with_the_prepared_paints() -> Result<(), String> {
    let tree = parse(OVAL_SVG)?;
    let node = first_path(&tree)?;
    let mut actual = canvas()?;
    if !render_if_supported_with_tree(
        node,
        Transform::identity(),
        &mut actual.as_mut(),
        Some(&tree),
    )? {
        return Err("source ellipse did not use the supported Skia path route".to_owned());
    }
    let expected = render_direct_oval(&tree, node, Transform::identity())?;
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

#[test]
fn ordinary_path_keeps_the_existing_cubic_preparation() -> Result<(), String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><path d="M8 8 C28 0 40 32 56 24 Z" fill="#e02020"/></svg>"##;
    let tree = parse(svg)?;
    let node = first_path(&tree)?;
    let mut old = canvas()?;
    let mut contextual = canvas()?;
    assert!(render_if_supported(
        node,
        Transform::identity(),
        &mut old.as_mut()
    )?);
    assert!(render_if_supported_with_tree(
        node,
        Transform::identity(),
        &mut contextual.as_mut(),
        Some(&tree),
    )?);
    assert_eq!(old.data(), contextual.data());
    Ok(())
}

#[test]
fn authored_clip_uses_the_existing_strict_renderer() -> Result<(), String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><defs><clipPath id="authored"><rect x="8" y="8" width="20" height="20"/></clipPath></defs><g clip-path="url(#authored)"><ellipse cx="16" cy="20" rx="8" ry="6" fill="#e02020"/></g></svg>"##;
    let tree = parse(svg)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let info = AuthoredClipInfo {
        ids: HashSet::from(["authored".to_owned()]),
        generated_ids: HashSet::new(),
        has_object_bounding_box: false,
    };
    let context = RenderContext::strict(&info, Some(&rasterizer));
    assert!(context.primitive_tree.is_none());
    let mut actual = canvas()?;
    SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut actual.as_mut(),
        &info,
        Some(&rasterizer),
    )?;
    let mut expected = canvas()?;
    resvg::render(&tree, Transform::identity(), &mut expected.as_mut());
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

#[test]
fn effect_group_keeps_the_strict_cubic_child_path() -> Result<(), String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><g opacity="0.5"><ellipse cx="16" cy="20" rx="8" ry="6" fill="#e02020"/></g></svg>"##;
    let tree = parse(svg)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut actual = canvas()?;
    SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut actual.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    )?;
    let mut strict = canvas()?;
    super::super::render_children_with_text(
        tree.root(),
        Transform::identity().pre_concat(tree.root().transform()),
        &mut strict.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    )?;
    assert_eq!(actual.data(), strict.data());
    Ok(())
}

#[test]
fn generated_viewport_clip_keeps_source_tree_context_for_ellipse_children() -> Result<(), String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><svg x="8" y="8" width="24" height="24"><ellipse cx="12" cy="12" rx="8" ry="6" fill="#e02020"/></svg></svg>"##;
    assert!(nested_clip_keeps_native_oval(svg)?);
    let edge_svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><svg x="0" y="8" width="24" height="24"><ellipse cx="4" cy="12" rx="4" ry="6" fill="#e02020"/></svg></svg>"##;
    assert!(!nested_clip_keeps_native_oval(edge_svg)?);
    let tree = parse(svg)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    let mut output = canvas()?;
    SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut output.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    )?;
    assert!(
        output
            .pixel(CENTER_SAMPLE.0, CENTER_SAMPLE.1)
            .is_some_and(|pixel| pixel.alpha() > 0)
    );
    Ok(())
}

fn nested_clip_keeps_native_oval(svg: &str) -> Result<bool, String> {
    let tree = parse(svg)?;
    let group = first_group(&tree)?;
    if group.clip_path().is_none() {
        return Err("nested viewport generated clip missing".to_owned());
    }
    let root_transform = Transform::identity().pre_concat(tree.root().transform());
    let transform = root_transform.pre_concat(group.transform());
    let mut target = canvas()?;
    let (crop, crop_transform) = nested_crop_context(group, transform, &mut target)?;
    let mut cropped = Pixmap::new(crop.width(), crop.height())
        .ok_or("failed to allocate nested viewport crop")?;
    let (path, path_transform) = first_group_path(group)?;
    let path_crop_transform = crop_transform.pre_concat(path_transform);
    let prepared = prepare_path(path)?.ok_or("nested viewport paints were not prepared")?;
    let prepared = prepare_source_oval(
        path,
        Some(&tree),
        path_crop_transform,
        &cropped.as_mut(),
        prepared,
    );
    Ok(prepared.path.is_oval().is_some())
}

fn first_group(tree: &usvg::Tree) -> Result<&usvg::Group, String> {
    tree.root()
        .children()
        .iter()
        .find_map(|node| match node {
            usvg::Node::Group(group) => Some(group.as_ref()),
            _ => None,
        })
        .ok_or_else(|| "nested viewport group missing".to_owned())
}

fn nested_crop_context(
    group: &usvg::Group,
    transform: Transform,
    target: &mut Pixmap,
) -> Result<(tiny_skia::IntRect, Transform), String> {
    super::super::layers::crop_context(group, transform, &target.as_mut())?
        .ok_or_else(|| "nested viewport crop missing".to_owned())
}

fn first_group_path(group: &usvg::Group) -> Result<(&usvg::Path, Transform), String> {
    first_path_with_transform(group.children(), Transform::identity())
        .ok_or_else(|| "nested viewport path missing".to_owned())
}

fn first_path_with_transform(
    nodes: &[usvg::Node],
    transform: Transform,
) -> Option<(&usvg::Path, Transform)> {
    nodes.iter().find_map(|node| match node {
        usvg::Node::Path(path) => Some((path.as_ref(), transform)),
        usvg::Node::Group(group) => {
            first_path_with_transform(group.children(), transform.pre_concat(group.transform()))
        }
        _ => None,
    })
}

fn uses_native_oval(svg: &str, transform: Transform) -> Result<bool, String> {
    let tree = parse(svg)?;
    let node = first_path(&tree)?;
    let prepared = prepare_path(node)?.ok_or("ellipse paints were not prepared")?;
    let mut output = canvas()?;
    let target = output.as_mut();
    let prepared = prepare_source_oval(node, Some(&tree), transform, &target, prepared);
    Ok(prepared.path.is_oval().is_some())
}

fn render_direct_oval(
    tree: &usvg::Tree,
    node: &usvg::Path,
    transform: Transform,
) -> Result<Pixmap, String> {
    let mut prepared = prepare_path(node)?.ok_or("ellipse paints were not prepared")?;
    let source = tree
        .source_ellipse(node)
        .ok_or("source ellipse metadata missing")?;
    prepared.path = geometry::source_oval(source, prepared.path.fill_type())
        .ok_or("source ellipse did not form a finite oval")?;
    let mut output = canvas()?;
    render_prepared_path(
        &mut output.as_mut(),
        &prepared,
        transform,
        node.paint_order(),
    )?;
    Ok(output)
}

fn parse(svg: &str) -> Result<usvg::Tree, String> {
    usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|error| error.to_string())
}

fn first_path(tree: &usvg::Tree) -> Result<&usvg::Path, String> {
    first_path_in(
        tree.root().children(),
        "ellipse/path fixture produced no root path",
    )
}

fn first_path_in<'a>(nodes: &'a [usvg::Node], missing: &str) -> Result<&'a usvg::Path, String> {
    nodes
        .iter()
        .find_map(|node| match node {
            usvg::Node::Path(path) => Some(path.as_ref()),
            _ => None,
        })
        .ok_or_else(|| missing.to_owned())
}

fn canvas() -> Result<Pixmap, String> {
    Pixmap::new(SIZE, SIZE).ok_or_else(|| "failed to allocate oval fixture pixmap".to_owned())
}
