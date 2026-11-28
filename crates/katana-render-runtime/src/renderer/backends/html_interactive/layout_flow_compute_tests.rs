use super::{CssStyle, Display, TaffyTree, compute_flow_layout, flow_stretch_heights};
use crate::renderer::backends::html_interactive::layout_flow_measure::leaf_style_for_flow;
use crate::renderer::backends::html_interactive::style::CssGridTrack;
use taffy::geometry::Size;
use taffy::prelude::{FlexWrap, Style};
use taffy::style::AlignItems;
use taffy::style_helpers::length;

#[test]
fn wrapped_flex_stretches_each_line_independently() -> Result<(), String> {
    let mut parent = CssStyle::browser_default();
    parent.display = Display::Flex;
    parent.flex_wrap = FlexWrap::Wrap;
    parent.gap = 12.0;
    for alignment in [None, Some(AlignItems::NORMAL), Some(AlignItems::STRETCH)] {
        parent.align_items = alignment;
        assert_flow_stretch(
            &parent,
            542.0,
            265.0,
            &[(106.6, 106.6), (90.0, 106.6), (81.8, 81.8)],
        )?;
    }
    Ok(())
}

#[test]
fn grid_stretches_items_to_the_tallest_item_in_each_row() -> Result<(), String> {
    let mut parent = CssStyle::browser_default();
    parent.display = Display::Grid;
    parent.grid_template_columns = vec![CssGridTrack::Length(50.0); 2];
    assert_flow_stretch(
        &parent,
        100.0,
        40.0,
        &[(20.0, 40.0), (40.0, 40.0), (10.0, 10.0)],
    )
}

#[test]
fn centered_flex_preserves_each_items_height() -> Result<(), String> {
    let mut parent = CssStyle::browser_default();
    parent.display = Display::Flex;
    parent.align_items = Some(AlignItems::CENTER);
    let mut tree = TaffyTree::<()>::new();
    tree.disable_rounding();
    let measurements = [
        measure_item(&mut tree, 40.0, 20.0)?,
        measure_item(&mut tree, 40.0, 40.0)?,
    ];
    let nodes = measurements.each_ref().map(|(node, _, _)| *node);
    compute_flow_layout(&mut tree, &nodes, &parent, 100.0, None)?;
    let short_item = tree.layout(nodes[0]).map_err(super::layout_error)?;
    assert_eq!(short_item.size.height, 20.0);
    assert_eq!(short_item.location.y, 10.0);
    assert_eq!(flow_stretch_heights(&tree, &parent, &measurements), None);
    Ok(())
}

#[test]
fn auto_height_uses_fractional_computed_height_for_flex_stretch() -> Result<(), String> {
    let mut parent = CssStyle::browser_default();
    parent.display = Display::Flex;
    let child = CssStyle::browser_default();
    let mut tree = TaffyTree::<()>::new();
    tree.disable_rounding();
    let node = tree
        .new_leaf(leaf_style_for_flow(
            child.clone(),
            40.0,
            20.0,
            100.0,
            &parent,
        ))
        .map_err(super::layout_error)?;
    compute_flow_layout(&mut tree, &[node], &parent, 100.0, Some(40.25))?;
    assert_eq!(
        flow_stretch_heights(&tree, &parent, &[(node, child, 20.0)]),
        Some(vec![40.25])
    );
    Ok(())
}

#[test]
fn layout_error_preserves_the_taffy_child_index_error() -> Result<(), String> {
    let mut tree = TaffyTree::<()>::new();
    let (node, _, _) = measure_item(&mut tree, 40.0, 20.0)?;
    assert_eq!(
        tree.child_at_index(node, 0).map_err(super::layout_error),
        Err(format!(
            "CSS flow layout failed: Index (is 0) should be < child_count (0) for parent node {node:?}"
        ))
    );
    Ok(())
}

fn assert_flow_stretch(
    parent: &CssStyle,
    available_width: f32,
    item_width: f32,
    heights: &[(f32, f32)],
) -> Result<(), String> {
    let mut tree = TaffyTree::<()>::new();
    tree.disable_rounding();
    let measurements = heights
        .iter()
        .map(|&(height, _)| measure_item(&mut tree, item_width, height))
        .collect::<Result<Vec<_>, _>>()?;
    let nodes = measurements
        .iter()
        .map(|(node, _, _)| *node)
        .collect::<Vec<_>>();
    let expected = heights.iter().map(|&(_, height)| height).collect();
    compute_flow_layout(&mut tree, &nodes, parent, available_width, None)?;
    assert_eq!(
        flow_stretch_heights(&tree, parent, &measurements),
        Some(expected)
    );
    Ok(())
}

fn measure_item(
    tree: &mut TaffyTree<()>,
    width: f32,
    height: f32,
) -> Result<(taffy::NodeId, CssStyle, f32), String> {
    let node = tree
        .new_leaf(Style {
            size: Size {
                width: length(width),
                height: length(height),
            },
            ..Style::default()
        })
        .map_err(super::layout_error)?;
    let mut css = CssStyle::browser_default();
    css.height = Some(height);
    Ok((node, css, height))
}
