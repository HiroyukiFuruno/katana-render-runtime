use super::super::super::super::html_document::HtmlDocumentNode;
use super::super::super::constants::MIN_LAYOUT_WIDTH;
use super::super::super::layout::HtmlLayoutRenderer;
use super::super::super::layout_flow_measure::{assign_leaf_height, item_style};
use super::super::super::style::CssStyle;
use super::super::super::types::LayoutContext;
use super::super::compute::{flow_stretch_heights, layout_error};
use taffy::prelude::TaffyTree;

type FlowMeasurement = (taffy::tree::NodeId, CssStyle, f32);

impl HtmlLayoutRenderer {
    pub(in crate::renderer::backends::html_interactive) fn remeasure_flow_heights(
        &self,
        tree: &mut TaffyTree<()>,
        nodes: &[taffy::tree::NodeId],
        items: &[&HtmlDocumentNode],
        layout: LayoutContext<'_>,
        list_mode: bool,
    ) -> Result<(), String> {
        self.measure_flow_items(tree, nodes, items, layout, list_mode)
            .and_then(|measurements| {
                let stretch_heights = flow_stretch_heights(tree, layout.style, &measurements);
                self.apply_flow_item_heights(tree, measurements, stretch_heights.as_deref())
                    .map_err(layout_error)
            })
    }

    fn apply_flow_item_heights(
        &self,
        tree: &mut TaffyTree<()>,
        measurements: Vec<FlowMeasurement>,
        stretch_heights: Option<&[f32]>,
    ) -> taffy::TaffyResult<()> {
        measurements.into_iter().enumerate().try_for_each(
            |(index, (node, css_style, measured_height))| {
                self.apply_flow_item_height(
                    tree,
                    node,
                    &css_style,
                    measured_height,
                    stretch_heights,
                    index,
                )
            },
        )
    }

    fn apply_flow_item_height(
        &self,
        tree: &mut TaffyTree<()>,
        node: taffy::tree::NodeId,
        css_style: &CssStyle,
        measured_height: f32,
        stretch_heights: Option<&[f32]>,
        index: usize,
    ) -> taffy::TaffyResult<()> {
        let height = if css_style.height.is_none() {
            stretch_heights.map_or(measured_height, |values| values[index])
        } else {
            measured_height
        };
        tree.style(node).cloned().and_then(|mut style| {
            assign_leaf_height(&mut style, css_style, height);
            tree.set_style(node, style)
        })
    }

    fn measure_flow_items(
        &self,
        tree: &TaffyTree<()>,
        nodes: &[taffy::tree::NodeId],
        items: &[&HtmlDocumentNode],
        layout: LayoutContext<'_>,
        list_mode: bool,
    ) -> Result<Vec<FlowMeasurement>, String> {
        nodes
            .iter()
            .zip(items)
            .map(|(&node, item)| {
                let width = tree.unrounded_layout(node).size.width.max(MIN_LAYOUT_WIDTH);
                self.measure_flow_item_height(item, width, layout, list_mode)
                    .map(|height| {
                        let css_style = item_style(item, layout.style, layout.width, items.len());
                        (node, css_style, height)
                    })
            })
            .collect()
    }
}
