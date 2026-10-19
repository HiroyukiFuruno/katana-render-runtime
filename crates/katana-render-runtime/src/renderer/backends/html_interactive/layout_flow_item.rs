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
mod tests {
    use super::super::types::DetailsContext;
    use super::HtmlLayoutRenderer;
    use super::{CssStyle, HtmlDocumentNode};
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use std::collections::HashMap;

    #[test]
    fn nested_flow_measurements_are_reused_within_one_frame() -> Result<(), String> {
        let root = nested_flow_node();
        let renderer = test_renderer();
        let style = CssStyle::browser_default();
        let first =
            renderer.measure_flow_node_height(&root, 300.0, &style, DetailsContext::NONE)?;
        let first_stats = renderer.flow_measurements.borrow().stats();
        let second =
            renderer.measure_flow_node_height(&root, 300.0, &style, DetailsContext::NONE)?;
        let second_stats = renderer.flow_measurements.borrow().stats();

        assert_eq!(first, second);
        assert!(first_stats.0 > 0, "nested remeasurement did not hit cache");
        assert_eq!(second_stats.0, first_stats.0 + 1);
        assert_eq!(second_stats.1, first_stats.1);
        Ok(())
    }

    #[test]
    fn list_item_container_measurement_uses_a_distinct_cache_entry() -> Result<(), String> {
        let node = list_item_node();
        let renderer = test_renderer();
        let style = CssStyle::browser_default();
        let ordinary =
            renderer.measure_flow_node_height(&node, 144.0, &style, DetailsContext::NONE)?;
        let ordinary_stats = renderer.flow_measurements.borrow().stats();
        let list_item = renderer.measure_flow_node_height_with_list_mode(
            &node,
            144.0,
            &style,
            DetailsContext::NONE,
            true,
        )?;
        let list_stats = renderer.flow_measurements.borrow().stats();

        assert_ne!(ordinary, list_item);
        assert!(
            list_stats.1 > ordinary_stats.1,
            "list mode reused ordinary measurement"
        );
        Ok(())
    }

    fn list_item_node() -> HtmlDocumentNode {
        HtmlDocumentNode::Element {
            node_id: 3,
            tag: "li".to_string(),
            attributes: vec![(
                "style".to_string(),
                "min-height:42px;padding:4px".to_string(),
            )],
            children: vec![HtmlDocumentNode::Text("Short".to_string())],
        }
    }

    fn nested_flow_node() -> HtmlDocumentNode {
        let nested = HtmlDocumentNode::Element {
            node_id: 2,
            tag: "section".to_string(),
            attributes: vec![("style".to_string(), "display:flex".to_string())],
            children: vec![HtmlDocumentNode::Text("Nested content".to_string())],
        };
        HtmlDocumentNode::Element {
            node_id: 1,
            tag: "main".to_string(),
            attributes: vec![(
                "style".to_string(),
                "display:flex;flex-direction:column".to_string(),
            )],
            children: vec![nested],
        }
    }

    fn test_renderer() -> HtmlLayoutRenderer {
        HtmlLayoutRenderer::new(
            HtmlBrowserViewport {
                width: 320,
                height: 240,
                device_scale_factor: 1.0,
            },
            0.0,
            &HashMap::new(),
            None,
        )
    }
}
