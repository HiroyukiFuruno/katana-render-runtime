use super::constants::{CONTROL_HEIGHT, DISABLED_CONTROL_OPACITY, MIN_LAYOUT_WIDTH};
use super::control_style::{
    button_style, button_width, details_is_open, summary_height, visible_details_children,
};
use super::document::attribute;
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::types::{
    ControlLayout, DetailsContext, ElementRenderContext, HitTargetKind, LayoutContext,
};

impl HtmlLayoutRenderer {
    pub(super) fn render_button(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let start = layout.y + layout.style.margin_top;
        let (x, button_width, height, style) = button_dimensions(element, layout);
        self.paint_button(
            element,
            ControlLayout {
                x,
                y: start,
                width: button_width,
                height,
                style: &style,
            },
        );
        start + height + style.margin_bottom
    }

    pub(super) fn render_details(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let start = layout.y + layout.style.margin_top;
        let x = layout.x + layout.style.margin_left;
        let width = (layout.width - layout.style.margin_left - layout.style.margin_right)
            .max(MIN_LAYOUT_WIDTH);
        let details_open = details_is_open(element.attributes);
        let content = visible_details_children(element.attributes, element.children);
        let (inner_x, inner_y, inner_width) = details_content_layout(x, start, width, layout.style);
        let box_start = self.svg.len();
        let bottom = self.render_nodes(
            &content,
            inner_x,
            inner_y,
            inner_width,
            layout.style,
            DetailsContext::from_open_state(element.node_id, details_open),
        );
        let height = details_height(bottom, start, layout.style);
        self.capture_stacking_context_children();
        let before_background = self.svg.len();
        self.insert_box(box_start, x, start, width, height, layout.style);
        self.record_stacking_background(self.svg.len() - before_background);
        start + height + layout.style.margin_bottom
    }

    pub(super) fn render_summary(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let control = summary_layout(layout);
        self.paint_summary(element.children, control, layout.details.open);
        self.push_summary_target(
            element.node_id,
            layout.details.node_id,
            control.x,
            control.y,
            control.width,
            control.height,
        );
        control.y + control.height + control.style.margin_bottom
    }

    fn push_summary_target(
        &mut self,
        node_id: u64,
        details_node_id: Option<u64>,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        if let Some(details_node_id) = details_node_id {
            self.push_target(
                node_id,
                x,
                y,
                width,
                height,
                HitTargetKind::Summary { details_node_id },
            );
        }
    }
}

fn details_content_layout(x: f32, start: f32, width: f32, style: &CssStyle) -> (f32, f32, f32) {
    let inner_x = x + style.border_left_width() + style.padding_left;
    let inner_y = start + style.border_top_width() + style.padding_top;
    let inner_width = (width
        - style.border_left_width()
        - style.border_right_width()
        - style.padding_left
        - style.padding_right)
        .max(MIN_LAYOUT_WIDTH);
    (inner_x, inner_y, inner_width)
}

fn details_height(bottom: f32, start: f32, style: &CssStyle) -> f32 {
    (bottom - start + style.padding_bottom + style.border_top_width() + style.border_bottom_width())
        .max(CONTROL_HEIGHT)
}

fn button_dimensions(
    element: ElementRenderContext<'_>,
    layout: LayoutContext<'_>,
) -> (f32, f32, f32, CssStyle) {
    let x = layout.x + layout.style.margin_left;
    let mut style = button_style(layout.style);
    let available_width =
        (layout.width - layout.style.margin_left - layout.style.margin_right).max(MIN_LAYOUT_WIDTH);
    let button_width = button_width(element.children, available_width, &style);
    let height = button_height(layout.style);
    apply_disabled_button_style(&mut style, element.attributes);
    (x, button_width, height, style)
}

fn button_height(style: &CssStyle) -> f32 {
    style.height.unwrap_or(CONTROL_HEIGHT).max(style.min_height)
}

fn apply_disabled_button_style(style: &mut CssStyle, attributes: &[(String, String)]) {
    if attribute(attributes, "disabled").is_some() {
        style.color = "#767676".to_string();
        style.border = Some("#767676".to_string());
        style.opacity = DISABLED_CONTROL_OPACITY;
    }
}

fn summary_layout(layout: LayoutContext<'_>) -> ControlLayout<'_> {
    ControlLayout {
        x: layout.x + layout.style.margin_left,
        y: layout.y + layout.style.margin_top,
        width: (layout.width - layout.style.margin_left - layout.style.margin_right)
            .max(MIN_LAYOUT_WIDTH),
        height: summary_height(layout.style),
        style: layout.style,
    }
}

#[cfg(test)]
mod tests {
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use crate::renderer::backends::html_document::HtmlDocumentNode;
    use crate::renderer::backends::html_interactive::constants::SELECT_HEIGHT;
    use crate::renderer::backends::html_interactive::layout::HtmlLayoutRenderer;
    use crate::renderer::backends::html_interactive::types::LayoutResult;
    use std::collections::HashMap;

    const SELECT_TEST_VIEWPORT_WIDTH: u32 = 320;
    const SELECT_TEST_VIEWPORT_HEIGHT: u32 = 240;

    #[test]
    fn input_layout_uses_attribute_value_when_session_seed_is_absent() -> Result<(), String> {
        let nodes = vec![HtmlDocumentNode::Element {
            node_id: 1,
            tag: "input".to_string(),
            attributes: vec![("value".to_string(), "fallback value".to_string())],
            children: Vec::new(),
        }];
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let layout = HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None)?;

        assert!(layout.svg.contains("fallback value"));
        Ok(())
    }

    #[test]
    fn select_layout_paints_one_closed_control_with_selected_option() -> Result<(), String> {
        let layout = render_select_layout()?;
        assert!(layout.svg.contains("標準"));
        assert!(!layout.svg.contains("未選択標準"));
        assert_default_select_box_height(&layout)
    }

    fn render_select_layout() -> Result<LayoutResult, String> {
        let option = |selected: bool, text: &str| HtmlDocumentNode::Element {
            node_id: 2,
            tag: "option".to_string(),
            attributes: if selected {
                vec![("selected".to_string(), "".to_string())]
            } else {
                Vec::new()
            },
            children: vec![HtmlDocumentNode::Text(text.to_string())],
        };
        let nodes = vec![HtmlDocumentNode::Element {
            node_id: 1,
            tag: "select".to_string(),
            attributes: Vec::new(),
            children: vec![option(false, "未選択"), option(true, "標準")],
        }];
        let viewport = HtmlBrowserViewport {
            width: SELECT_TEST_VIEWPORT_WIDTH,
            height: SELECT_TEST_VIEWPORT_HEIGHT,
            device_scale_factor: 1.0,
        };
        HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None)
    }

    fn assert_default_select_box_height(layout: &LayoutResult) -> Result<(), String> {
        let select = layout
            .element_boxes
            .iter()
            .find(|element| element.node_id == 1)
            .ok_or("select element box is missing")?;
        assert_eq!(select.height, SELECT_HEIGHT);
        Ok(())
    }
}
