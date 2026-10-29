use super::NativeArcPlan;
use resvg::usvg::{self, Node};
use skia_safe::PathVerb;

const ARC: &str = "M0,15.835777126099707 a108,15.835777126099707 0,0,0 216,0 a108,15.835777126099707 0,0,0 -216,0 l0,90.00193770959581 a108,15.835777126099707 0,0,0 216,0 l0,-90.00193770959581";
const CYLINDER_WIDTH: f32 = 216.0;

fn scene(attributes: &str) -> Result<usvg::Tree, String> {
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><g id='outer' {attributes}><path id='arc' d='{ARC}' fill='red' stroke='blue' stroke-width='2'/></g></svg>"
    );
    usvg::Tree::from_str(&svg, &usvg::Options::default()).map_err(|error| error.to_string())
}

fn nodes(tree: &usvg::Tree) -> Result<(&usvg::Group, &usvg::Path), String> {
    let Some(Node::Group(group)) = tree.node_by_id("outer") else {
        return Err("group missing".to_owned());
    };
    let Some(Node::Path(path)) = tree.node_by_id("arc") else {
        return Err("path missing".to_owned());
    };
    Ok((group, path))
}

#[test]
fn actual_arc_is_selected_and_bounds_include_stroke() -> Result<(), String> {
    let tree = scene("")?;
    let (group, path) = nodes(&tree)?;
    let plan = NativeArcPlan::build(&tree)?;
    let prepared = plan.path(path).ok_or("candidate not selected")?;
    assert!(prepared.path.verbs().contains(&PathVerb::Conic));
    let bounds = plan.group_bounds(group).ok_or("group bounds missing")?;
    let ink = super::bounds::ink_bounds(prepared).ok_or("ink bounds missing")?;
    assert!(bounds.x() <= ink.x() && bounds.y() <= ink.y());
    assert!(bounds.right() >= ink.right() && bounds.bottom() >= ink.bottom());
    assert!(bounds.x() < 0.0 && bounds.right() > CYLINDER_WIDTH);
    Ok(())
}

#[test]
fn opacity_branch_keeps_legacy_path() -> Result<(), String> {
    let tree = scene("opacity='0.5'")?;
    let (group, path) = nodes(&tree)?;
    let plan = NativeArcPlan::build(&tree)?;
    assert!(plan.group_bounds(group).is_none());
    assert!(plan.path(path).is_none());
    Ok(())
}

#[test]
fn group_transform_expands_parent_bounds() -> Result<(), String> {
    let tree = scene("transform='translate(-20 4) scale(2 0.5)'")?;
    let (group, path) = nodes(&tree)?;
    let plan = NativeArcPlan::build(&tree)?;
    let local = plan.group_bounds(group).ok_or("group bounds missing")?;
    let transformed = super::bounds::transform_bounds(local, group.transform())
        .ok_or("transformed bounds unavailable")?;
    let parent = plan
        .group_bounds(tree.root())
        .ok_or("root bounds missing")?;
    assert!(plan.path(path).is_some());
    assert!(parent.x() <= transformed.x() && parent.right() >= transformed.right());
    assert!(parent.y() <= transformed.y() && parent.bottom() >= transformed.bottom());
    Ok(())
}

#[test]
fn clipped_group_retains_its_native_candidate() -> Result<(), String> {
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><defs><clipPath id='c'><rect width='100' height='100'/></clipPath></defs><g id='outer' clip-path='url(#c)'><path id='arc' d='{ARC}'/></g></svg>"
    );
    let tree =
        usvg::Tree::from_str(&svg, &usvg::Options::default()).map_err(|error| error.to_string())?;
    let (group, path) = nodes(&tree)?;
    let plan = NativeArcPlan::build(&tree)?;
    assert!(group.clip_path().is_some());
    assert!(plan.group_bounds(group).is_some());
    assert!(plan.path(path).is_some());
    Ok(())
}

#[test]
fn dashed_arc_is_not_planned() -> Result<(), String> {
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><path id='arc' d='{ARC}' stroke='blue' stroke-dasharray='2 2'/></svg>"
    );
    let tree =
        usvg::Tree::from_str(&svg, &usvg::Options::default()).map_err(|error| error.to_string())?;
    let Some(Node::Path(path)) = tree.node_by_id("arc") else {
        return Err("path missing".to_owned());
    };
    assert!(NativeArcPlan::build(&tree)?.path(path).is_none());
    Ok(())
}
