use super::{
    AuthoredClipInfo, SvgCompositeRenderer, arc_plan::NativeArcPlan, tests::FallbackTextRasterizer,
};
use resvg::usvg;
use tiny_skia::{Pixmap, Transform};

const SIZE: u32 = 400;
const ARC: &str = "M0,15.835777126099707 a108,15.835777126099707 0,0,0 216,0 a108,15.835777126099707 0,0,0 -216,0 l0,90.00193770959581 a108,15.835777126099707 0,0,0 216,0 l0,-90.00193770959581";

fn tree(content: &str) -> Result<usvg::Tree, String> {
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{SIZE}' height='{SIZE}'>{content}</svg>"
    );
    usvg::Tree::from_str(&svg, &usvg::Options::default()).map_err(|error| error.to_string())
}

fn render(tree: &usvg::Tree, full: bool) -> Result<Pixmap, String> {
    let mut pixmap = Pixmap::new(SIZE, SIZE).ok_or("pixmap unavailable")?;
    let info = AuthoredClipInfo::default();
    let text = Some(&FallbackTextRasterizer as &dyn super::TextRasterizer);
    if full {
        SvgCompositeRenderer::render_full_with_text(
            tree,
            Transform::identity(),
            &mut pixmap.as_mut(),
            &info,
            text,
        )?;
    } else {
        SvgCompositeRenderer::render_with_text(
            tree,
            Transform::identity(),
            &mut pixmap.as_mut(),
            &info,
            text,
        )?;
    }
    Ok(pixmap)
}

fn arc_tree(nested_clip: bool) -> Result<usvg::Tree, String> {
    let shape = format!("<path id='arc' d='{ARC}' fill='red' stroke='blue' stroke-width='2'/>");
    let body = if nested_clip {
        format!(
            "<defs><clipPath id='outer'><rect x='-10' y='-10' width='250' height='160'/></clipPath><clipPath id='inner'><rect x='-10' y='-10' width='250' height='160'/></clipPath></defs><g id='outer-group' clip-path='url(#outer)'><g id='inner-group' clip-path='url(#inner)'>{shape}</g></g>"
        )
    } else {
        shape
    };
    tree(&format!(
        "<g transform='translate(300 40) scale(-1 1)'>{body}</g>"
    ))
}

#[test]
fn nested_clips_preserve_planned_arc_with_negative_transform() -> Result<(), String> {
    let unclipped = arc_tree(false)?;
    let clipped = arc_tree(true)?;
    let plan = NativeArcPlan::build(&clipped)?;
    let Some(usvg::Node::Path(path)) = clipped.node_by_id("arc") else {
        return Err("arc missing".into());
    };
    assert!(plan.path(path).is_some());
    assert_clip_bounds(&clipped, &plan)?;
    let actual = render(&clipped, true)?;
    let expected = render(&unclipped, true)?;
    assert!(actual.data().iter().any(|&byte| byte != 0));
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

fn assert_clip_bounds(tree: &usvg::Tree, plan: &NativeArcPlan<'_>) -> Result<(), String> {
    for id in ["outer-group", "inner-group"] {
        let Some(usvg::Node::Group(group)) = tree.node_by_id(id) else {
            return Err(format!("{id} missing"));
        };
        let extra = plan.group_bounds(group).ok_or("planned bounds missing")?;
        let bounds = super::groups::planned_group_bounds(group, Some(extra))?;
        assert!(bounds.x() <= extra.x() && bounds.right() >= extra.right());
        assert!(bounds.y() <= extra.y() && bounds.bottom() >= extra.bottom());
    }
    Ok(())
}

#[test]
fn unsupported_effects_keep_strict_path_and_pixels() -> Result<(), String> {
    let defs = "<defs><filter id='blur'><feGaussianBlur stdDeviation='1'/></filter><mask id='mask'><rect width='400' height='400' fill='white'/></mask></defs>";
    for effect in ["opacity='0.5'", "filter='url(#blur)'", "mask='url(#mask)'"] {
        let tree = tree(&format!(
            "{defs}<g {effect}><path id='arc' d='{ARC}' fill='red'/></g>"
        ))?;
        assert_strict_path(&tree)?;
    }
    Ok(())
}

fn assert_strict_path(tree: &usvg::Tree) -> Result<(), String> {
    let plan = NativeArcPlan::build(tree)?;
    let Some(usvg::Node::Path(path)) = tree.node_by_id("arc") else {
        return Err("arc missing".into());
    };
    assert!(plan.path(path).is_none());
    assert_eq!(render(tree, true)?.data(), render(tree, false)?.data());
    Ok(())
}

#[test]
fn ordinary_path_is_unchanged_by_full_plan() -> Result<(), String> {
    let tree = tree("<path id='plain' d='M20 20 L100 30 L40 100 Z' fill='red' stroke='blue'/>")?;
    let plan = NativeArcPlan::build(&tree)?;
    let Some(usvg::Node::Path(path)) = tree.node_by_id("plain") else {
        return Err("path missing".into());
    };
    assert!(plan.path(path).is_none());
    assert_eq!(render(&tree, true)?.data(), render(&tree, false)?.data());
    Ok(())
}

#[test]
fn nonfinite_accumulated_transform_uses_strict_leaf_path() -> Result<(), String> {
    let tree = arc_tree(false)?;
    let plan = NativeArcPlan::build(&tree)?;
    let Some(node @ usvg::Node::Path(path)) = tree.node_by_id("arc") else {
        return Err("arc missing".into());
    };
    assert!(plan.path(path).is_some());
    let info = AuthoredClipInfo::default();
    let text = FallbackTextRasterizer;
    let planned = super::RenderContext {
        provenance: &info,
        text: Some(&text),
        plan: Some(&plan),
    };
    let strict = super::RenderContext {
        provenance: &info,
        text: Some(&text),
        plan: None,
    };
    let mut actual = Pixmap::new(SIZE, SIZE).ok_or("pixmap unavailable")?;
    let mut expected = Pixmap::new(SIZE, SIZE).ok_or("pixmap unavailable")?;
    let transform = Transform::from_scale(f32::INFINITY, 1.0);
    super::nodes::render_node(node, transform, &mut actual.as_mut(), &planned)?;
    super::nodes::render_node(node, transform, &mut expected.as_mut(), &strict)?;
    assert_eq!(actual.data(), expected.data());
    Ok(())
}
