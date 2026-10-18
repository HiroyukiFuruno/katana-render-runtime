use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::layout::HtmlLayoutRenderer;
use super::layout_flow_measure::{
    assign_leaf_height, item_style, leaf_style_for_flow, measured_widths,
};
use super::style::CssStyle;
use super::types::{DetailsContext, LayoutContext};
use taffy::prelude::{Display, TaffyTree};

#[path = "layout_flow_compute.rs"]
mod compute;

use compute::{
    compute_flow_layout, compute_root_layout, flow_stretch_heights, is_visible_layout_item,
    layout_error,
};

type FlowMeasurement = (taffy::tree::NodeId, CssStyle, f32);

impl HtmlLayoutRenderer {
    pub(super) fn render_flow_children(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
        available_height: Option<f32>,
    ) -> Result<f32, String> {
        match layout.style.display {
            Display::Flex | Display::Grid => {
                self.render_taffy_flow(children, layout, available_height)
            }
            Display::Block | Display::FlowRoot => Ok(self.render_nodes(
                children,
                layout.x,
                layout.y,
                layout.width,
                layout.style,
                layout.details,
            )),
            Display::None => Ok(layout.y),
        }
    }

    fn render_taffy_flow(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
        available_height: Option<f32>,
    ) -> Result<f32, String> {
        let (x, y, width) = (layout.x, layout.y, layout.width);
        let (style, details) = (layout.style, layout.details);
        let items = children
            .iter()
            .filter(|node| is_visible_layout_item(node, style))
            .collect::<Vec<_>>();
        if items.is_empty() {
            return Ok(y);
        }
        let mut tree = TaffyTree::<()>::new();
        tree.disable_rounding();
        let nodes = self.build_flow_nodes(&mut tree, &items, width, style, details)?;
        let root = compute_flow_layout(&mut tree, &nodes, style, width, available_height)?;
        self.remeasure_flow_heights(&mut tree, &nodes, &items, width, style, details)?;
        compute_root_layout(&mut tree, root, width, available_height)?;
        self.paint_flow_items(&tree, &nodes, &items, (x, y), style, details)
    }

    fn build_flow_nodes(
        &mut self,
        tree: &mut TaffyTree<()>,
        items: &[&HtmlDocumentNode],
        width: f32,
        style: &CssStyle,
        details: DetailsContext,
    ) -> Result<Vec<taffy::tree::NodeId>, String> {
        let item_styles = items
            .iter()
            .map(|item| item_style(item, style, width, items.len()))
            .collect::<Vec<_>>();
        let widths = measured_widths(items, &item_styles, width, style);
        let mut nodes = Vec::with_capacity(items.len());
        for ((item, item_style), measure_width) in items.iter().zip(item_styles).zip(widths) {
            let height = self.measure_flow_node_height(item, measure_width, style, details)?;
            let node = tree
                .new_leaf(leaf_style_for_flow(
                    item_style,
                    measure_width,
                    height,
                    width,
                    style,
                ))
                .map_err(layout_error)?;
            nodes.push(node);
        }
        Ok(nodes)
    }

    fn paint_flow_items(
        &mut self,
        tree: &TaffyTree<()>,
        nodes: &[taffy::tree::NodeId],
        items: &[&HtmlDocumentNode],
        origin: (f32, f32),
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        let (x, y) = origin;
        let mut bottom = y;
        for (node, item) in nodes.iter().zip(items) {
            let layout = tree.layout(*node).map_err(layout_error)?;
            let item_bottom = self.render_flow_node(
                item,
                LayoutContext::new(
                    x + layout.location.x,
                    y + layout.location.y,
                    layout.size.width.max(MIN_LAYOUT_WIDTH),
                    inherited,
                    details,
                ),
                Some(layout.size.height),
            );
            bottom = bottom.max(item_bottom);
        }
        Ok(bottom)
    }

    fn remeasure_flow_heights(
        &self,
        tree: &mut TaffyTree<()>,
        nodes: &[taffy::tree::NodeId],
        items: &[&HtmlDocumentNode],
        available_width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> Result<(), String> {
        let measurements =
            self.measure_flow_items(tree, nodes, items, available_width, inherited, details)?;
        let stretch_heights = flow_stretch_heights(tree, inherited, &measurements)?;
        for (index, (node, css_style, measured_height)) in measurements.into_iter().enumerate() {
            let height = if css_style.height.is_none() {
                stretch_heights
                    .as_ref()
                    .map_or(measured_height, |heights| heights[index])
            } else {
                measured_height
            };
            let mut style = tree.style(node).map_err(layout_error)?.clone();
            assign_leaf_height(&mut style, &css_style, height);
            tree.set_style(node, style).map_err(layout_error)?;
        }
        Ok(())
    }

    fn measure_flow_items(
        &self,
        tree: &TaffyTree<()>,
        nodes: &[taffy::tree::NodeId],
        items: &[&HtmlDocumentNode],
        available_width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> Result<Vec<FlowMeasurement>, String> {
        let mut measurements = Vec::with_capacity(nodes.len());
        for (node, item) in nodes.iter().zip(items) {
            let width = tree
                .layout(*node)
                .map_err(layout_error)?
                .size
                .width
                .max(MIN_LAYOUT_WIDTH);
            let height = self.measure_flow_node_height(item, width, inherited, details)?;
            let css_style = item_style(item, inherited, available_width, items.len());
            measurements.push((*node, css_style, height));
        }
        Ok(measurements)
    }
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, DetailsContext, Display, HtmlDocumentNode, LayoutContext};
    use super::{HtmlLayoutRenderer, layout_error};
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use std::collections::HashMap;

    #[test]
    fn none_and_empty_flow_paths_preserve_the_current_position() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let mut style = CssStyle::browser_default();
        style.display = Display::None;
        assert_empty_flow_position(&mut renderer, &style, 7.0);

        style.display = Display::FlowRoot;
        assert_empty_flow_position(&mut renderer, &style, 8.0);

        style.display = Display::Flex;
        let whitespace = [HtmlDocumentNode::Text(" \n ".to_string())];
        assert_eq!(
            renderer.render_flow_children(
                &whitespace,
                LayoutContext::new(0.0, 9.0, 100.0, &style, DetailsContext::NONE),
                None,
            ),
            Ok(9.0)
        );
        assert_eq!(layout_error("boom"), "CSS flow layout failed: boom");
    }

    fn assert_empty_flow_position(renderer: &mut HtmlLayoutRenderer, style: &CssStyle, y: f32) {
        assert_eq!(
            renderer.render_flow_children(
                &[],
                LayoutContext::new(0.0, y, 100.0, style, DetailsContext::NONE),
                None,
            ),
            Ok(y)
        );
    }
}
