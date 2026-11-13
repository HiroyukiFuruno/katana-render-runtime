use super::super::super::html_document::HtmlDocumentNode;
use super::super::constants::MIN_LAYOUT_WIDTH;
use super::super::layout::HtmlLayoutRenderer;
use super::super::layout_flow_measure::{item_style, leaf_style_for_flow, measured_widths};
use super::super::style::CssStyle;
use super::super::types::{DetailsContext, LayoutContext};
use super::compute::{is_visible_layout_item, layout_error};
use taffy::prelude::TaffyTree;

#[path = "layout_flow_remeasure.rs"]
mod remeasure;

impl HtmlLayoutRenderer {
    pub(in crate::renderer::backends::html_interactive) fn render_list_flow_children(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: super::super::types::LayoutContext<'_>,
        available_height: Option<f32>,
        ordered: bool,
    ) -> Result<f32, String> {
        self.render_taffy_flow(children, layout, available_height, Some(ordered))
    }

    pub(super) fn measure_flow_item_height(
        &self,
        item: &HtmlDocumentNode,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
        list_mode: bool,
    ) -> Result<f32, String> {
        self.measure_flow_node_height_with_list_mode(item, width, inherited, details, list_mode)
    }

    pub(super) fn build_flow_nodes(
        &mut self,
        tree: &mut TaffyTree<()>,
        items: &[&HtmlDocumentNode],
        width: f32,
        style: &CssStyle,
        details: DetailsContext,
        list_mode: bool,
    ) -> Result<Vec<taffy::tree::NodeId>, String> {
        let item_styles = items
            .iter()
            .map(|item| item_style(item, style, width, items.len()))
            .collect::<Vec<_>>();
        let widths = measured_widths(items, &item_styles, width, style);
        let layout = LayoutContext::new(0.0, 0.0, width, style, details);
        items
            .iter()
            .zip(item_styles)
            .zip(widths)
            .map(|((item, item_style), measure_width)| {
                self.build_flow_node(tree, item, item_style, measure_width, layout, list_mode)
            })
            .collect()
    }

    fn build_flow_node(
        &mut self,
        tree: &mut TaffyTree<()>,
        item: &HtmlDocumentNode,
        item_style: CssStyle,
        measure_width: f32,
        layout: LayoutContext<'_>,
        list_mode: bool,
    ) -> Result<taffy::tree::NodeId, String> {
        let height = self.measure_flow_item_height(
            item,
            measure_width,
            layout.style,
            layout.details,
            list_mode,
        )?;
        tree.new_leaf(leaf_style_for_flow(
            item_style,
            measure_width,
            height,
            layout.width,
            layout.style,
        ))
        .map_err(layout_error)
    }

    pub(super) fn paint_flow_items(
        &mut self,
        tree: &TaffyTree<()>,
        nodes: &[taffy::tree::NodeId],
        items: &[&HtmlDocumentNode],
        layout_context: LayoutContext<'_>,
        ordered_list: Option<bool>,
    ) -> f32 {
        let (x, y) = (layout_context.x, layout_context.y);
        let mut bottom = y;
        for (index, (node, item)) in nodes.iter().zip(items).enumerate() {
            let layout = tree.unrounded_layout(*node);
            let marker = ListItemMarker::for_index(ordered_list, index);
            let item_bottom = self.render_flow_node_with_list_marker(
                item,
                LayoutContext::new(
                    x + layout.location.x,
                    y + layout.location.y,
                    layout.size.width.max(MIN_LAYOUT_WIDTH),
                    layout_context.style,
                    layout_context.details,
                ),
                Some(layout.size.height),
                marker.filter(|_| is_direct_list_item(item)),
            );
            bottom = bottom.max(item_bottom);
        }
        bottom
    }
}

#[derive(Clone, Copy)]
pub(in crate::renderer::backends::html_interactive) struct ListItemMarker {
    pub(in crate::renderer::backends::html_interactive) ordered: bool,
    pub(in crate::renderer::backends::html_interactive) index: usize,
}

impl ListItemMarker {
    pub(super) fn for_index(ordered_list: Option<bool>, zero_based_index: usize) -> Option<Self> {
        ordered_list.map(|ordered| Self {
            ordered,
            index: zero_based_index + 1,
        })
    }
}

pub(super) fn flow_items<'a>(
    children: &'a [HtmlDocumentNode],
    inherited: &CssStyle,
    list_mode: bool,
) -> Vec<&'a HtmlDocumentNode> {
    children
        .iter()
        .filter(|child| !list_mode || is_direct_list_item(child))
        .filter(|child| is_visible_layout_item(child, inherited))
        .collect()
}

pub(super) fn is_direct_list_item(node: &HtmlDocumentNode) -> bool {
    matches!(node, HtmlDocumentNode::Element { tag, .. } if tag == "li")
}
