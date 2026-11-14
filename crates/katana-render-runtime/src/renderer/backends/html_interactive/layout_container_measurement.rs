use super::{
    ContainerGeometry, ContainingBlock, CssPosition, CssStyle, DetailsContext, HtmlBrowserViewport,
    HtmlDocumentNode, HtmlLayoutRenderer, LayoutContext, Rc, container_height,
};

impl HtmlLayoutRenderer {
    pub(super) fn resolve_container_containing_block(
        &mut self,
        children: &[HtmlDocumentNode],
        geometry: &ContainerGeometry,
        style: &CssStyle,
        details: DetailsContext,
    ) -> Option<ContainingBlock> {
        if style.position == CssPosition::Static && !style.has_transform {
            return None;
        }
        let height = style
            .height
            .map(|height| style.outer_height(height))
            .unwrap_or_else(|| {
                let measured =
                    self.measure_auto_container_height(children, geometry, style, details);
                self.accept_auto_container_height(measured, style)
            });
        Some(ContainingBlock {
            owner_node_id: self.current_element_owner(),
            x: geometry.box_x,
            y: geometry.start,
            width: geometry.box_width,
            height,
            establishes_fixed_containing_block: style.has_transform,
        })
    }

    pub(super) fn accept_auto_container_height(
        &mut self,
        measured: Result<f32, String>,
        style: &CssStyle,
    ) -> f32 {
        measured.unwrap_or_else(|error| {
            self.layout_error = Some(error);
            style.minimum_outer_height()
        })
    }

    pub(super) fn measure_auto_container_height(
        &self,
        children: &[HtmlDocumentNode],
        geometry: &ContainerGeometry,
        style: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        let renderer = self.new_auto_measurement_renderer(geometry);
        let bottom = self
            .render_children_for_auto_measurement(renderer, children, geometry, style, details)?;
        Ok(container_height(bottom, 0.0, style))
    }

    pub(super) fn new_auto_measurement_renderer(&self, geometry: &ContainerGeometry) -> Self {
        let viewport = HtmlBrowserViewport {
            width: geometry.inner_width.ceil().max(1.0) as u32,
            height: 1,
            device_scale_factor: 1.0,
        };
        Self::new_with_measurement_cache(
            viewport,
            0.0,
            &self.input_values,
            self.focused_input,
            Rc::clone(&self.flow_measurements),
        )
    }

    pub(super) fn render_children_for_auto_measurement(
        &self,
        mut renderer: Self,
        children: &[HtmlDocumentNode],
        geometry: &ContainerGeometry,
        style: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        let mut child_style = style.clone();
        child_style.percentage_height_basis = None;
        let content_start = style.border_top_width() + style.padding_top;
        renderer
            .render_flow_children(
                children,
                LayoutContext::new(
                    0.0,
                    content_start,
                    geometry.inner_width,
                    &child_style,
                    details,
                ),
                None,
            )
            .and_then(|bottom| renderer.ensure_layout_succeeded().map(|()| bottom))
    }
}
