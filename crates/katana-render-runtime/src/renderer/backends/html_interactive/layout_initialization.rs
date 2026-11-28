use super::super::svg::svg_header;
use super::{FlowMeasurementCache, HtmlBrowserViewport, HtmlLayoutRenderer};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

impl HtmlLayoutRenderer {
    #[cfg(test)]
    pub(in super::super) fn new(
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
    ) -> Self {
        Self::new_with_measurement_cache(
            viewport,
            scroll_y,
            input_values,
            focused_input,
            Rc::new(RefCell::new(FlowMeasurementCache::default())),
        )
    }

    pub(in super::super) fn new_with_measurement_cache(
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
        flow_measurements: Rc<RefCell<FlowMeasurementCache>>,
    ) -> Self {
        Self::new_with_clickable_nodes(
            viewport,
            scroll_y,
            input_values,
            focused_input,
            &std::collections::HashSet::new(),
            flow_measurements,
        )
    }

    pub(in super::super) fn new_with_clickable_nodes(
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
        clickable_nodes: &std::collections::HashSet<u64>,
        flow_measurements: Rc<RefCell<FlowMeasurementCache>>,
    ) -> Self {
        let (svg, document_paint_start) = initial_svg(viewport);
        Self {
            scroll_y,
            svg,
            input_values: input_values.clone(),
            focused_input,
            viewport_height: viewport.logical_height(),
            viewport_width: viewport.logical_width(),
            clickable_nodes: clickable_nodes.clone(),
            document_paint_start,
            flow_measurements,
            ..Self::default()
        }
    }
}

fn initial_svg(viewport: HtmlBrowserViewport) -> (String, usize) {
    let svg = svg_header(viewport);
    let document_paint_start = svg.len();
    (svg, document_paint_start)
}
