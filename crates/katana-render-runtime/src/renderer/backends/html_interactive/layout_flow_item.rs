use super::super::html_browser::HtmlBrowserViewport;
use super::super::html_document::HtmlDocumentNode;
use super::layout::HtmlLayoutRenderer;
use super::layout_measurement_cache::FlowMeasurementKey;
use super::style::CssStyle;
use super::types::{DetailsContext, ElementRenderContext, LayoutContext};
use std::rc::Rc;

impl HtmlLayoutRenderer {
    pub(super) fn render_flow_node(
        &mut self,
        node: &HtmlDocumentNode,
        layout: LayoutContext<'_>,
        assigned_height: Option<f32>,
    ) -> f32 {
        match node {
            HtmlDocumentNode::Text(text) => {
                self.render_text(text, layout.x, layout.y, layout.width, layout.style)
            }
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
            ),
        }
    }

    fn render_flow_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
        assigned_height: Option<f32>,
    ) -> f32 {
        let style = self.prepare_flow_style(element, layout, assigned_height);
        let (sticky_x, sticky_y) = self.flow_element_position(&style, layout);
        let scope = self.begin_element_paint_scope(element.node_id, &style);
        let bottom = self.render_styled_element(
            element,
            LayoutContext {
                style: &style,
                x: sticky_x,
                y: sticky_y,
                ..layout
            },
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

    pub(super) fn measure_flow_node_height(
        &self,
        node: &HtmlDocumentNode,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        let cache_key = FlowMeasurementKey::for_node(node, width);
        if let Some(height) = cache_key.and_then(|key| self.flow_measurements.borrow_mut().get(key))
        {
            return Ok(height);
        }
        let height = self.measure_uncached_flow_node_height(node, width, inherited, details)?;
        if let Some(key) = cache_key {
            self.flow_measurements.borrow_mut().insert(key, height);
        }
        Ok(height)
    }

    fn measure_uncached_flow_node_height(
        &self,
        node: &HtmlDocumentNode,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        let viewport = HtmlBrowserViewport {
            width: width.ceil().max(1.0) as u32,
            height: 1,
            device_scale_factor: 1.0,
        };
        let mut renderer = Self::new_with_measurement_cache(
            viewport,
            0.0,
            &self.input_values,
            self.focused_input,
            Rc::clone(&self.flow_measurements),
        );
        let bottom = renderer.render_flow_node(
            node,
            LayoutContext::new(0.0, 0.0, width, inherited, details),
            None,
        );
        renderer.ensure_layout_succeeded()?;
        Ok(bottom.max(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlLayoutRenderer;
    use super::{CssStyle, DetailsContext, HtmlBrowserViewport, HtmlDocumentNode};
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
