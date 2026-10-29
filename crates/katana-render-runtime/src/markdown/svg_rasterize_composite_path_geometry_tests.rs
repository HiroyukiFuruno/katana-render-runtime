use super::{convert, convert_node, parse_original};
use crate::markdown::svg_rasterize::{
    composite::{SvgCompositeRenderer, tests::FallbackTextRasterizer},
    provenance::AuthoredClipInfo,
};
use resvg::usvg;
use skia_safe::{PathFillType, PathVerb};
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};

const ARC: &str = "M10 20 A10 5 0 0 1 30 20 L10 20 Z";
const JA03_CYLINDER_ARC: &str = "M0,15.835777126099707 a108,15.835777126099707 0,0,0 216,0 a108,15.835777126099707 0,0,0 -216,0 l0,90.00193770959581 a108,15.835777126099707 0,0,0 216,0 l0,-90.00193770959581";
const TRANSLATION_X: f32 = 5.0;
const SIZE: u32 = 64;
const INSIDE_POINT: (u32, u32) = (20, 18);
const OUTSIDE_POINT: (u32, u32) = (11, 18);
const CLIPPED_POINT: (u32, u32) = (32, 18);
const RGBA_CHANNELS: usize = 4;
const ALPHA_CHANNEL: usize = 3;

fn path_for(d: &str, extra: &str) -> Result<usvg::Path, String> {
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{SIZE}' height='{SIZE}'><path id='p' d='{d}' {extra}/></svg>"
    );
    let tree =
        usvg::Tree::from_str(&svg, &usvg::Options::default()).map_err(|error| error.to_string())?;
    match tree.node_by_id("p") {
        Some(usvg::Node::Path(path)) => Ok(path.as_ref().clone()),
        _ => Err("path missing".to_owned()),
    }
}

#[test]
fn arc_uses_skia_conics_and_even_odd_fill() -> Result<(), String> {
    let node = path_for(ARC, "fill-rule='evenodd'")?;
    assert_eq!(node.original_arc(), Some(ARC));
    let path = convert_node(&node)?;
    assert!(path.verbs().contains(&PathVerb::Conic));
    assert_eq!(path.fill_type(), PathFillType::EvenOdd);
    Ok(())
}

#[test]
fn ja03_cylinder_strict_geometry_keeps_legacy_bounds() -> Result<(), String> {
    let node = path_for(JA03_CYLINDER_ARC, "")?;
    assert_eq!(node.original_arc(), Some(JA03_CYLINDER_ARC));
    let parsed = parse_original(JA03_CYLINDER_ARC, None).ok_or("Skia rejected cylinder")?;
    assert!(parsed.is_finite());
    assert!(parsed.verbs().contains(&PathVerb::Conic));
    let selected = convert_node(&node)?;
    let candidate = parsed.compute_tight_bounds();
    let legacy = node.bounding_box();
    println!(
        "{}",
        serde_json::json!({
            "candidate": [candidate.left, candidate.top, candidate.right, candidate.bottom],
            "legacy": [legacy.x(), legacy.y(), legacy.right(), legacy.bottom()],
            "selectedVerbs": format!("{:?}", selected.verbs()),
        })
    );
    assert!(!selected.verbs().contains(&PathVerb::Conic));
    Ok(())
}

#[test]
fn ordinary_path_and_invalid_suffix_use_existing_geometry() -> Result<(), String> {
    for d in ["M10 10 L30 10 L30 30 Z", "M10 20 A10 5 0 0 1 30 20 L"] {
        let node = path_for(d, "")?;
        assert!(node.original_arc().is_none());
        assert_eq!(
            convert_node(&node)?.verbs(),
            convert(node.data(), None)?.verbs()
        );
    }
    Ok(())
}

#[test]
fn invalid_and_nul_svg_are_rejected_without_panic() {
    assert!(parse_original("M0 0 A", None).is_none());
    assert!(parse_original("M0 0\0 A1 1 0 0 1 2 2", None).is_none());
}

#[test]
fn rejected_source_falls_back_to_existing_geometry() -> Result<(), String> {
    let mut node = path_for(ARC, "")?;
    for invalid in ["M0 0 A", "M0 0\0 A1 1 0 0 1 2 2"] {
        node.set_original_arc(invalid);
        assert_eq!(
            convert_node(&node)?.verbs(),
            convert(node.data(), None)?.verbs()
        );
    }
    Ok(())
}

#[test]
fn clones_share_metadata_until_geometry_is_replaced() -> Result<(), String> {
    let node = path_for(ARC, "")?;
    let mut cloned = node.clone();
    assert_eq!(cloned.original_arc(), Some(ARC));
    let transformed = cloned
        .data()
        .clone()
        .transform(usvg::Transform::from_translate(TRANSLATION_X, 0.0))
        .ok_or("failed to transform path")?;
    cloned.data = Arc::new(transformed);
    assert!(cloned.original_arc().is_none());
    assert_eq!(node.original_arc(), Some(ARC));
    assert_eq!(
        convert_node(&cloned)?.verbs(),
        convert(cloned.data(), None)?.verbs()
    );
    Ok(())
}

fn scene(use_reference: bool, clipped: bool) -> String {
    let shape = if use_reference {
        "<use href='#source'/>".to_owned()
    } else {
        format!("<path d='{ARC}'/>")
    };
    let clip = if clipped {
        "clip-path='url(#window)'"
    } else {
        ""
    };
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{SIZE}' height='{SIZE}'><defs><path id='source' d='{ARC}'/><clipPath id='window'><rect x='10' y='14' width='10' height='8'/></clipPath></defs><g transform='translate(3 4) scale(1.2 0.8)' {clip}>{shape}</g></svg>"
    )
}

fn find_arc(group: &usvg::Group) -> Option<&usvg::Path> {
    group.children().iter().find_map(|node| match node {
        usvg::Node::Path(path) if path.original_arc() == Some(ARC) => Some(path.as_ref()),
        usvg::Node::Group(group) => find_arc(group),
        _ => None,
    })
}

fn render_scene(svg: &str) -> Result<Pixmap, String> {
    let tree =
        usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|error| error.to_string())?;
    let node = find_arc(tree.root()).ok_or("arc source missing from rendered tree")?;
    assert!(convert_node(node)?.verbs().contains(&PathVerb::Conic));
    let mut pixmap = Pixmap::new(SIZE, SIZE).ok_or("pixmap unavailable")?;
    let rasterizer = FallbackTextRasterizer;
    SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut pixmap.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    )?;
    Ok(pixmap)
}

#[test]
fn use_and_direct_arc_match_under_transform_and_clip() -> Result<(), String> {
    let mut counts = Vec::new();
    for clipped in [false, true] {
        let direct = render_scene(&scene(false, clipped))?;
        let reused = render_scene(&scene(true, clipped))?;
        assert_eq!(direct.data(), reused.data());
        assert_sample_pixels(&direct, clipped)?;
        counts.push(
            direct
                .data()
                .chunks_exact(RGBA_CHANNELS)
                .filter(|pixel| pixel[ALPHA_CHANNEL] != 0)
                .count(),
        );
    }
    assert!(counts[1] > 0 && counts[1] < counts[0]);
    Ok(())
}

fn assert_sample_pixels(pixmap: &Pixmap, clipped: bool) -> Result<(), String> {
    let alpha = |point: (u32, u32)| pixmap.pixel(point.0, point.1).map(|pixel| pixel.alpha());
    assert!(alpha(INSIDE_POINT).ok_or("missing inside pixel")? > 0);
    assert_eq!(alpha(OUTSIDE_POINT).ok_or("missing outside pixel")?, 0);
    assert_eq!(
        alpha(CLIPPED_POINT).ok_or("missing clipped pixel")? > 0,
        !clipped
    );
    Ok(())
}
