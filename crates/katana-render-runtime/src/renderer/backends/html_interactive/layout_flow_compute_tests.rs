use super::{CssStyle, Display, TaffyTree, compute_flow_layout, flow_stretch_heights};
use crate::renderer::backends::html_interactive::style::CssGridTrack;
use taffy::geometry::Size;
use taffy::prelude::{FlexWrap, Style};
use taffy::style_helpers::length;

#[test]
fn wrapped_flex_stretches_each_line_independently() -> Result<(), String> {
    let mut parent = CssStyle::browser_default();
    parent.display = Display::Flex;
    parent.flex_wrap = FlexWrap::Wrap;
    parent.gap = 12.0;
    assert_flow_stretch(
        &parent,
        542.0,
        265.0,
        &[(106.6, 106.6), (90.0, 106.6), (81.8, 81.8)],
    )
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

fn assert_flow_stretch(
    parent: &CssStyle,
    available_width: f32,
    item_width: f32,
    heights: &[(f32, f32)],
) -> Result<(), String> {
    let mut tree = TaffyTree::<()>::new();
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
        flow_stretch_heights(&tree, parent, &measurements)?,
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
    Ok((node, CssStyle::browser_default(), height))
}
