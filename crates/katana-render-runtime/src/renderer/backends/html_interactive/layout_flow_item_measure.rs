use super::super::super::html_browser::HtmlBrowserViewport;
use super::super::super::html_document::HtmlDocumentNode;
use super::super::layout::HtmlLayoutRenderer;
use super::super::layout_measurement_cache::FlowMeasurementKey;
use super::super::style::CssStyle;
use super::super::types::DetailsContext;
use super::super::types::LayoutContext;
use std::rc::Rc;

impl HtmlLayoutRenderer {
    pub(in crate::renderer::backends::html_interactive) fn measure_flow_node_height(
        &self,
        node: &HtmlDocumentNode,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        self.measure_flow_node_height_with_list_mode(node, width, inherited, details, false)
    }

    pub(in crate::renderer::backends::html_interactive) fn measure_flow_node_height_with_list_mode(
        &self,
        node: &HtmlDocumentNode,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
        list_item_container: bool,
    ) -> Result<f32, String> {
        let cache_key = FlowMeasurementKey::for_node(node, width, list_item_container);
        if let Some(height) = cache_key.and_then(|key| self.flow_measurements.borrow_mut().get(key))
        {
            return Ok(height);
        }
        let measured = self.measure_uncached_flow_node_height(
            node,
            width,
            inherited,
            details,
            list_item_container,
        );
        self.cache_measured_flow_height(cache_key, measured)
    }

    pub(super) fn cache_measured_flow_height(
        &self,
        key: Option<FlowMeasurementKey>,
        measured: Result<f32, String>,
    ) -> Result<f32, String> {
        measured.inspect(|&height| {
            if let Some(key) = key {
                self.flow_measurements.borrow_mut().insert(key, height);
            }
        })
    }

    fn measure_uncached_flow_node_height(
        &self,
        node: &HtmlDocumentNode,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
        list_item_container: bool,
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
        let layout = LayoutContext::new(0.0, 0.0, width, inherited, details);
        let bottom = if list_item_container {
            renderer.render_flow_node_with_measurement_mode(node, layout, None, None, true)
        } else {
            renderer.render_flow_node(node, layout, None)
        };
        renderer.ensure_layout_succeeded()?;
        Ok(bottom.max(1.0))
    }
}
