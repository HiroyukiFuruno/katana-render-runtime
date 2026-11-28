use super::super::html_browser::HtmlBrowserViewport;
use super::super::html_document::HtmlDocumentNode;
use super::layout::HtmlLayoutRenderer;
use super::layout_measurement_cache::FlowMeasurementCache;
use super::types::LayoutResult;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

impl HtmlLayoutRenderer {
    #[cfg(test)]
    pub(super) fn render_with_clickable_nodes(
        nodes: &[HtmlDocumentNode],
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
        clickable_nodes: &std::collections::HashSet<u64>,
    ) -> Result<LayoutResult, String> {
        Self::render_with_clickable_nodes_and_cache(
            nodes,
            viewport,
            (0.0, scroll_y),
            input_values,
            focused_input,
            clickable_nodes,
            Rc::new(RefCell::new(FlowMeasurementCache::default())),
        )
    }

    pub(super) fn render_with_clickable_nodes_and_cache(
        nodes: &[HtmlDocumentNode],
        viewport: HtmlBrowserViewport,
        scroll_offset: (f32, f32),
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
        clickable_nodes: &std::collections::HashSet<u64>,
        flow_measurements: Rc<RefCell<FlowMeasurementCache>>,
    ) -> Result<LayoutResult, String> {
        render_cached_layout(
            nodes,
            viewport,
            scroll_offset,
            input_values,
            focused_input,
            clickable_nodes,
            flow_measurements,
        )
    }
}

fn render_cached_layout(
    nodes: &[HtmlDocumentNode],
    viewport: HtmlBrowserViewport,
    scroll_offset: (f32, f32),
    input_values: &HashMap<u64, String>,
    focused_input: Option<u64>,
    clickable_nodes: &std::collections::HashSet<u64>,
    flow_measurements: Rc<RefCell<FlowMeasurementCache>>,
) -> Result<LayoutResult, String> {
    let _measurement_scope = super::text_metrics::TextMeasurementCacheScope::enter();
    let _style_declaration_scope = super::style_declaration_cache::Scope::enter_if_absent();
    let mut renderer = HtmlLayoutRenderer::new_with_clickable_nodes(
        viewport,
        scroll_offset.1,
        input_values,
        focused_input,
        clickable_nodes,
        flow_measurements,
    );
    renderer.scroll_x = scroll_offset.0;
    let bottom = render_layout_body(&mut renderer, nodes, viewport);
    renderer.into_layout_result(bottom)
}

fn render_layout_body(
    renderer: &mut HtmlLayoutRenderer,
    nodes: &[HtmlDocumentNode],
    viewport: HtmlBrowserViewport,
) -> f32 {
    let width = viewport
        .logical_width()
        .max(super::constants::MIN_LAYOUT_WIDTH);
    let root_style = super::style::CssStyle::browser_default_for_viewport(
        viewport.logical_width(),
        viewport.logical_height(),
    );
    renderer.viewport_overflow = Some(super::layout_viewport_overflow::viewport_overflow_policy(
        nodes,
        &root_style,
    ));
    renderer.render_nodes(
        nodes,
        0.0,
        0.0,
        width,
        &root_style,
        super::types::DetailsContext::NONE,
    )
}
