use super::{CssStyle, Display, TaffyTree, compute_flow_layout, flow_stretch_heights};
use taffy::geometry::Size;
use taffy::prelude::{FlexWrap, Style};
use taffy::style_helpers::length;

#[test]
fn wrapped_flex_stretches_each_line_independently() -> Result<(), String> {
    let mut tree = TaffyTree::<()>::new();
    let mut parent = CssStyle::browser_default();
    parent.display = Display::Flex;
    parent.flex_wrap = FlexWrap::Wrap;
    parent.gap = 12.0;
    let mut measurements = Vec::new();
    for height in [106.6, 90.0, 81.8] {
        let node = tree
            .new_leaf(Style {
                size: Size {
                    width: length(265.0_f32),
                    height: length(height),
                },
                ..Style::default()
            })
            .map_err(super::layout_error)?;
        measurements.push((node, CssStyle::browser_default(), height));
    }
    let nodes = measurements
        .iter()
        .map(|(node, _, _)| *node)
        .collect::<Vec<_>>();
    compute_flow_layout(&mut tree, &nodes, &parent, 542.0, None)?;
    assert_eq!(
        flow_stretch_heights(&tree, &parent, &measurements)?,
        Some(vec![106.6, 106.6, 81.8])
    );
    Ok(())
}
