use super::super::html_document::HtmlDocumentNode;
use super::constants::{LIST_MARKER_WIDTH, MIN_LAYOUT_WIDTH, RULE_VERTICAL_INSET};
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::svg::escape_xml;
use super::types::{DetailsContext, LayoutContext};
use taffy::prelude::Display;

#[path = "layout_structures_flow.rs"]
mod flow;

impl HtmlLayoutRenderer {
    pub(super) fn render_rule(&mut self, x: f32, y: f32, width: f32, style: &CssStyle) -> f32 {
        let line_y = y + style.margin_top + RULE_VERTICAL_INSET;
        let x = x + style.margin_left;
        let width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        self.svg.push_str(&format!(
            r#"<line x1="{x}" y1="{line_y}" x2="{}" y2="{line_y}" stroke="{}" stroke-width="1"/>"#,
            x + width,
            escape_xml(style.border.as_deref().unwrap_or("#c8cdd2"))
        ));
        line_y + RULE_VERTICAL_INSET + style.margin_bottom
    }

    pub(super) fn render_list(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
        ordered: bool,
    ) -> f32 {
        if style.list_style_none {
            return self.render_container(children, x, y, width, style, DetailsContext::NONE);
        }
        if matches!(style.display, Display::Grid | Display::Flex) {
            return self.render_list_container(
                children,
                LayoutContext::new(x, y, width, style, DetailsContext::NONE),
                ordered,
            );
        }
        self.render_block_list(
            children,
            LayoutContext::new(x, y, width, style, DetailsContext::NONE),
            ordered,
        )
    }

    fn render_block_list(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
        ordered: bool,
    ) -> f32 {
        let style = layout.style;
        let current = layout.y + style.margin_top + style.padding_top + style.border_top_width();
        let x = layout.x + style.margin_left + style.padding_left + style.border_left_width()
            - LIST_MARKER_WIDTH;
        let outer = layout.width - style.margin_left - style.margin_right;
        let width = (style.content_width(outer) + LIST_MARKER_WIDTH).max(MIN_LAYOUT_WIDTH);
        self.render_list_children(children, x, current, width, style, ordered)
            + style.padding_bottom
            + style.border_bottom_width()
            + style.margin_bottom
    }

    fn render_list_children(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        mut current: f32,
        width: f32,
        style: &CssStyle,
        ordered: bool,
    ) -> f32 {
        let mut index = 1usize;
        for child in children {
            if let Some((attributes, items)) = list_item(child) {
                let mut item_style = CssStyle::from_element("li", attributes, style);
                item_style.resolve_text_line_height(items);
                current = self.render_list_item_contents(
                    items,
                    x,
                    current,
                    width,
                    &item_style,
                    Some((ordered, index)),
                );
                index += 1;
            }
        }
        current
    }

    pub(super) fn render_list_item(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        if style.list_style_none {
            return self.render_container(children, x, y, width, style, DetailsContext::NONE);
        }
        self.render_list_item_contents(children, x, y, width, style, Some((false, 1)))
    }

    fn render_list_item_contents(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
        marker: Option<(bool, usize)>,
    ) -> f32 {
        let marker_width = self.list_marker_width(marker, x, y, style);
        let result = self.render_flow_children(
            children,
            LayoutContext::new(
                x + marker_width,
                y,
                (width - marker_width).max(MIN_LAYOUT_WIDTH),
                style,
                DetailsContext::NONE,
            ),
            style.children_height(),
        );
        accept_list_flow_result(&mut self.layout_error, result, y)
    }

    fn list_marker_width(
        &mut self,
        marker: Option<(bool, usize)>,
        x: f32,
        y: f32,
        style: &CssStyle,
    ) -> f32 {
        if style.list_style_none {
            return 0.0;
        }
        if let Some((ordered, index)) = marker {
            self.paint_list_marker(ordered, index, x, y, style);
        }
        LIST_MARKER_WIDTH
    }
}

fn accept_list_flow_result(
    layout_error: &mut Option<String>,
    result: Result<f32, String>,
    start: f32,
) -> f32 {
    result.unwrap_or_else(|error| {
        *layout_error = Some(error);
        start
    })
}

type ListItem<'a> = (&'a [(String, String)], &'a [HtmlDocumentNode]);

fn list_item(node: &HtmlDocumentNode) -> Option<ListItem<'_>> {
    let HtmlDocumentNode::Element {
        tag,
        attributes,
        children,
        ..
    } = node
    else {
        return None;
    };
    (tag == "li").then_some((attributes, children))
}

#[cfg(test)]
mod tests {
    use super::HtmlLayoutRenderer;
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use crate::renderer::backends::html_document::HtmlDocumentNode;
    use crate::renderer::backends::html_interactive::style::CssStyle;
    use std::collections::HashMap;

    #[test]
    fn list_flow_errors_are_recorded_at_the_item_start() {
        use super::accept_list_flow_result;

        let mut layout_error = None;

        let bottom = accept_list_flow_result(&mut layout_error, Err("layout failed".into()), 12.0);

        assert_eq!(bottom, 12.0);
        assert_eq!(layout_error.as_deref(), Some("layout failed"));
    }

    fn list_item(node_id: u64) -> HtmlDocumentNode {
        HtmlDocumentNode::Element {
            node_id,
            tag: "li".to_string(),
            attributes: Vec::new(),
            children: vec![HtmlDocumentNode::Text("item".to_string())],
        }
    }

    fn viewport() -> HtmlBrowserViewport {
        HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        }
    }

    #[test]
    fn list_markers_are_skipped_when_list_style_is_none() {
        let mut style = CssStyle::browser_default();
        style.list_style_none = true;
        let mut renderer = HtmlLayoutRenderer::new(viewport(), 0.0, &HashMap::new(), None);

        let nodes = [list_item(1)];
        renderer.render_list(&nodes, 0.0, 0.0, 160.0, &style, false);

        assert!(!renderer.svg.contains("•"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn list_items_resolve_normal_height_for_their_actual_fonts() {
        let mut style = CssStyle::browser_default();
        style.font_family = "-apple-system, Hiragino Sans, sans-serif".to_string();
        let nodes = [HtmlDocumentNode::Element {
            node_id: 1,
            tag: "li".to_string(),
            attributes: Vec::new(),
            children: vec![HtmlDocumentNode::Text("日本語".to_string())],
        }];
        let mut renderer = HtmlLayoutRenderer::new(viewport(), 0.0, &HashMap::new(), None);
        let bottom = renderer.render_list(&nodes, 0.0, 0.0, 160.0, &style, true);
        assert_eq!(bottom, 24.0);
    }

    #[test]
    fn marker_width_is_zero_when_list_style_is_none() {
        let mut style = CssStyle::browser_default();
        style.list_style_none = true;
        let mut renderer = HtmlLayoutRenderer::new(viewport(), 0.0, &HashMap::new(), None);
        let svg_length = renderer.svg.len();

        assert_eq!(renderer.list_marker_width(None, 0.0, 0.0, &style), 0.0);
        assert_eq!(renderer.svg.len(), svg_length);
    }

    #[test]
    fn marker_width_reserves_space_without_painting_when_marker_is_absent() {
        let style = CssStyle::browser_default();
        let mut renderer = HtmlLayoutRenderer::new(viewport(), 0.0, &HashMap::new(), None);
        let svg_length = renderer.svg.len();

        assert_eq!(
            renderer.list_marker_width(None, 0.0, 0.0, &style),
            super::LIST_MARKER_WIDTH
        );
        assert_eq!(renderer.svg.len(), svg_length);
    }

    #[test]
    fn list_markers_are_painted_when_enabled() {
        let mut style = CssStyle::browser_default();
        style.list_style_none = false;
        let mut renderer = HtmlLayoutRenderer::new(viewport(), 0.0, &HashMap::new(), None);

        let nodes = [list_item(1)];
        renderer.render_list(&nodes, 0.0, 0.0, 160.0, &style, false);
        assert!(renderer.svg.contains(">•<"));
        assert!(renderer.svg.contains(">item<"));
    }

    #[test]
    fn outside_marker_advances_and_gap_align_for_digit_counts_and_bullets() {
        let style = CssStyle::browser_default();
        let mut renderer = HtmlLayoutRenderer::new(viewport(), 0.0, &HashMap::new(), None);
        let gutter_x = 40.0;
        let marker_gap = super::super::text_metrics::text_width(" ", &style);

        for (ordered, index, marker) in [
            (true, 9, "9."),
            (true, 10, "10."),
            (true, 100, "100."),
            (false, 1, "•"),
        ] {
            let marker_advance = super::super::text_metrics::text_width(marker, &style);
            let expected_x = gutter_x + super::LIST_MARKER_WIDTH - marker_gap - marker_advance;
            let start = renderer.svg.len();

            renderer.paint_list_marker(ordered, index, gutter_x, 0.0, &style);

            let painted = &renderer.svg[start..];
            assert!(painted.contains(&format!(">{marker}</text>")), "{painted}");
            assert!(
                painted_text_x(painted)
                    .is_some_and(|actual_x| (actual_x - expected_x).abs() < 0.001),
                "{painted}"
            );
        }
    }

    fn painted_text_x(svg: &str) -> Option<f32> {
        svg.split_once("<text x=\"")?
            .1
            .split_once('"')?
            .0
            .parse()
            .ok()
    }
}
