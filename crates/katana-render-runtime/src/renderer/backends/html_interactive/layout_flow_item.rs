use super::super::html_document::HtmlDocumentNode;
use super::layout::HtmlLayoutRenderer;
use super::layout_flow::ListItemMarker;
use super::style::CssStyle;
use super::types::{ElementRenderContext, LayoutContext};

#[path = "layout_flow_item_measure.rs"]
mod measure;

impl HtmlLayoutRenderer {
    pub(super) fn render_flow_node(
        &mut self,
        node: &HtmlDocumentNode,
        layout: LayoutContext<'_>,
        assigned_height: Option<f32>,
    ) -> f32 {
        self.render_flow_node_with_list_marker(node, layout, assigned_height, None)
    }

    pub(super) fn render_flow_node_with_list_marker(
        &mut self,
        node: &HtmlDocumentNode,
        layout: LayoutContext<'_>,
        assigned_height: Option<f32>,
        marker: Option<ListItemMarker>,
    ) -> f32 {
        self.render_flow_node_with_measurement_mode(node, layout, assigned_height, marker, false)
    }

    fn render_flow_node_with_measurement_mode(
        &mut self,
        node: &HtmlDocumentNode,
        layout: LayoutContext<'_>,
        assigned_height: Option<f32>,
        marker: Option<ListItemMarker>,
        list_item_container: bool,
    ) -> f32 {
        match node {
            HtmlDocumentNode::Text(text) => self.render_flow_text_node(text, layout),
            HtmlDocumentNode::Element {
                node_id,
                tag,
                attributes,
                children,
            } => self.render_flow_element(
                ElementRenderContext {
                    node_id: *node_id,
                    tag,
                    attributes,
                    children,
                },
                layout,
                assigned_height,
                marker,
                list_item_container,
            ),
        }
    }

    fn render_flow_text_node(&mut self, text: &str, layout: LayoutContext<'_>) -> f32 {
        self.render_text(text, layout.x, layout.y, layout.width, layout.style)
    }

    fn render_flow_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
        assigned_height: Option<f32>,
        marker: Option<ListItemMarker>,
        list_item_container: bool,
    ) -> f32 {
        let style = self.prepare_flow_style(element, layout, assigned_height);
        let (sticky_x, sticky_y) = self.flow_element_position(&style, layout);
        let scope = self.begin_element_paint_scope(element.node_id, &style);
        let element_layout = LayoutContext {
            style: &style,
            x: sticky_x,
            y: sticky_y,
            ..layout
        };
        let bottom = render_flow_element_contents(
            self,
            element,
            element_layout,
            marker,
            list_item_container,
        );
        self.finish_element_paint_scope(scope, element.node_id, &style);
        layout.y + (bottom - sticky_y)
    }

    fn prepare_flow_style(
        &self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
        assigned_height: Option<f32>,
    ) -> CssStyle {
        let mut style = CssStyle::from_element(element.tag, element.attributes, layout.style);
        style.resolve_text_line_height(element.children);
        style.assign_outer_width(layout.width);
        if let Some(height) = assigned_height {
            style.assign_margin_box_height(height);
        }
        style
    }

    fn flow_element_position(&self, style: &CssStyle, layout: LayoutContext<'_>) -> (f32, f32) {
        let sticky_y = if style.position == super::style::CssPosition::Sticky {
            self.sticky_y(style, layout.y)
        } else {
            layout.y
        };
        let sticky_x = if style.position == super::style::CssPosition::Sticky {
            self.sticky_x(style, layout.x, layout.width)
        } else {
            layout.x
        };
        (sticky_x, sticky_y)
    }
}

fn render_flow_element_contents(
    renderer: &mut HtmlLayoutRenderer,
    element: ElementRenderContext<'_>,
    layout: LayoutContext<'_>,
    marker: Option<ListItemMarker>,
    list_item_container: bool,
) -> f32 {
    match (marker, list_item_container, element.tag) {
        (None, true, "li") => super::layout_element::render_styled_list_item_for_measurement(
            renderer, element, layout,
        ),
        (Some(marker), _, "li") => {
            renderer.render_styled_list_item_element(element, layout, marker)
        }
        _ => renderer.render_styled_element(element, layout),
    }
}

#[cfg(test)]
#[path = "layout_flow_item_tests.rs"]
mod tests;
