use super::{CssStyle, HtmlLayoutRenderer, TableCell};
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use std::collections::HashMap;

#[test]
fn anonymous_table_cells_do_not_create_dom_bounds() {
    let mut renderer = HtmlLayoutRenderer::new(
        HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        },
        0.0,
        &HashMap::new(),
        None,
    );
    let cell = TableCell {
        tag: "td".to_string(),
        attributes: Vec::new(),
        row_attributes: Vec::new(),
        row_group_tag: None,
        row_group_attributes: Vec::new(),
        element_ids: None,
        children: Vec::new(),
    };
    let style = CssStyle::browser_default();

    renderer.record_table_element_boxes(&cell, None, &style, &style, (0.0, 0.0, 10.0, 10.0));

    assert!(renderer.element_boxes.is_empty());
}
