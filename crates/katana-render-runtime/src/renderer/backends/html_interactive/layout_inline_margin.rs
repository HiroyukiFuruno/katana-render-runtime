use super::super::super::super::html_document::HtmlDocumentNode;
use super::super::super::layout::HtmlLayoutRenderer;
use super::super::super::style::{CssFloat, CssPosition, CssStyle};
use super::super::super::types::DetailsContext;
use super::super::state::InlineFlowState;
use taffy::prelude::Display;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BlockMargins {
    pub(super) top: f32,
    pub(super) bottom: f32,
}

struct MarginRenderContext<'a> {
    nodes: &'a [HtmlDocumentNode],
    index: usize,
    inherited: &'a CssStyle,
    details: DetailsContext,
    inline: &'a mut InlineFlowState,
    previous_block_bottom_margin: &'a mut f32,
    previous_block_collapsible: &'a mut bool,
}

pub(super) fn block_margins(node: &HtmlDocumentNode, inherited: &CssStyle) -> Option<BlockMargins> {
    let HtmlDocumentNode::Element {
        tag, attributes, ..
    } = node
    else {
        return None;
    };
    let style = CssStyle::from_element(tag, attributes, inherited);
    block_margins_for_style(node, &style)
}

fn block_margins_for_style(node: &HtmlDocumentNode, style: &CssStyle) -> Option<BlockMargins> {
    let HtmlDocumentNode::Element { children, .. } = node else {
        return None;
    };
    (matches!(
        style.display,
        Display::Block | Display::FlowRoot | Display::Flex | Display::Grid
    ) && !style.inline_block
        && style.position != CssPosition::Absolute
        && style.position != CssPosition::Fixed
        && style.float == CssFloat::None)
        .then_some(BlockMargins {
            top: super::super::super::layout_block_margin::collapsed_margin(
                style.margin_top,
                super::super::super::layout_block_margin::leading_child_margin(children, style),
            ),
            bottom: super::super::super::layout_block_margin::collapsed_margin(
                style.margin_bottom,
                super::super::super::layout_block_margin::trailing_child_margin(children, style),
            ),
        })
}

pub(super) fn collapsed_margin_overlap(previous_bottom: f32, current_top: f32) -> f32 {
    previous_bottom + current_top - collapsed_margin(previous_bottom, current_top)
}

pub(super) fn parent_allows_collapse(style: &CssStyle) -> bool {
    matches!(style.display, Display::Block | Display::FlowRoot)
}

impl HtmlLayoutRenderer {
    pub(crate) fn render_nodes(
        &mut self,
        nodes: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> f32 {
        let mut inline = InlineFlowState::new(x, y, width);
        let mut previous_block_bottom_margin = 0.0;
        let mut previous_block_collapsible = false;
        for index in 0..nodes.len() {
            render_margin_node(
                self,
                &mut MarginRenderContext {
                    nodes,
                    index,
                    inherited,
                    details,
                    inline: &mut inline,
                    previous_block_bottom_margin: &mut previous_block_bottom_margin,
                    previous_block_collapsible: &mut previous_block_collapsible,
                },
            );
        }
        inline.bottom()
    }
}

fn render_margin_node(renderer: &mut HtmlLayoutRenderer, context: &mut MarginRenderContext<'_>) {
    prepare_inline_line(renderer, context);
    let had_inline_content = context.inline.has_items;
    adjust_margin_start(
        context.nodes,
        context.index,
        context.inherited,
        had_inline_content,
        *context.previous_block_collapsible,
        *context.previous_block_bottom_margin,
        context.inline,
    );
    renderer.render_inline_or_block_node(
        context.nodes,
        context.index,
        context.inherited,
        context.details,
        context.inline,
    );
    remember_margin_state(
        context.nodes,
        context.index,
        context.inherited,
        had_inline_content,
        context.previous_block_bottom_margin,
        context.previous_block_collapsible,
    );
}

fn prepare_inline_line(renderer: &HtmlLayoutRenderer, context: &mut MarginRenderContext<'_>) {
    if !context.inline.has_items {
        context.inline.resolve_line_metrics(
            &context.nodes[context.index..],
            context.inherited,
            &renderer.clickable_nodes,
        );
    }
}

fn adjust_margin_start(
    nodes: &[HtmlDocumentNode],
    index: usize,
    inherited: &CssStyle,
    had_inline_content: bool,
    previous_block_collapsible: bool,
    previous_block_bottom_margin: f32,
    inline: &mut InlineFlowState,
) {
    if !had_inline_content
        && previous_block_collapsible
        && parent_allows_collapse(inherited)
        && let Some(current) = block_margins(&nodes[index], inherited)
    {
        inline.y -= collapsed_margin_overlap(previous_block_bottom_margin, current.top);
    }
}

fn remember_margin_state(
    nodes: &[HtmlDocumentNode],
    index: usize,
    inherited: &CssStyle,
    had_inline_content: bool,
    previous_block_bottom_margin: &mut f32,
    previous_block_collapsible: &mut bool,
) {
    if matches!(&nodes[index], HtmlDocumentNode::Text(text) if text.chars().all(super::super::super::layout_text::is_css_whitespace))
    {
        return;
    }
    if let HtmlDocumentNode::Element {
        tag, attributes, ..
    } = &nodes[index]
    {
        let style = CssStyle::from_element(tag, attributes, inherited);
        if style.display == Display::None {
            return;
        }
        if !had_inline_content && let Some(current) = block_margins_for_style(&nodes[index], &style)
        {
            *previous_block_bottom_margin = current.bottom;
            *previous_block_collapsible = true;
            return;
        }
    }
    *previous_block_bottom_margin = 0.0;
    *previous_block_collapsible = false;
}

fn collapsed_margin(previous_bottom: f32, current_top: f32) -> f32 {
    if previous_bottom >= 0.0 && current_top >= 0.0 {
        previous_bottom.max(current_top)
    } else if previous_bottom <= 0.0 && current_top <= 0.0 {
        previous_bottom.min(current_top)
    } else {
        previous_bottom + current_top
    }
}

#[cfg(test)]
mod tests {
    use super::collapsed_margin_overlap;
    use super::{CssStyle, HtmlDocumentNode};

    const TEST_NODE_ID: u64 = 51;

    #[test]
    fn sibling_collapse_includes_a_containers_leading_descendant() -> Result<(), String> {
        let child = HtmlDocumentNode::Element {
            node_id: 44,
            tag: "div".to_string(),
            attributes: vec![("style".to_string(), "margin:8px 0".to_string())],
            children: vec![HtmlDocumentNode::Text("tab".to_string())],
        };
        let container = HtmlDocumentNode::Element {
            node_id: 43,
            tag: "div".to_string(),
            attributes: Vec::new(),
            children: vec![child],
        };
        let margins = super::block_margins(&container, &CssStyle::browser_default())
            .ok_or("container block margins missing")?;
        assert_eq!(margins.top, 8.0);
        assert_eq!(collapsed_margin_overlap(8.0, margins.top), 8.0);
        assert_collapsed_descendant_gap(container)
    }

    fn assert_collapsed_descendant_gap(container: HtmlDocumentNode) -> Result<(), String> {
        let mut renderer = heading_list_renderer()?;
        let previous = HtmlDocumentNode::Element {
            node_id: 45,
            tag: "div".to_string(),
            attributes: vec![("style".to_string(), "margin-bottom:8px".to_string())],
            children: vec![HtmlDocumentNode::Text("form".to_string())],
        };
        renderer.render_nodes(
            &[previous, container],
            0.0,
            0.0,
            576.0,
            &CssStyle::browser_default(),
            super::super::super::super::types::DetailsContext::NONE,
        );
        let previous = find_box(&renderer, 45, "previous")?;
        let child = find_box(&renderer, 44, "leading child")?;
        assert!((child.y - previous.y - previous.height - 8.0).abs() < 0.01);
        Ok(())
    }

    #[test]
    fn inline_block_margins_do_not_join_block_margin_collapse() {
        let node = HtmlDocumentNode::Element {
            node_id: 42,
            tag: "span".to_string(),
            attributes: vec![(
                "style".to_string(),
                "display:inline-block;margin-top:14px".to_string(),
            )],
            children: vec![HtmlDocumentNode::Text("badge".to_string())],
        };
        assert!(super::block_margins(&node, &CssStyle::browser_default()).is_none());
    }

    #[test]
    fn remembers_block_margin_after_one_style_resolution() {
        let node = margin_node("margin:8px 0", vec![HtmlDocumentNode::Text("child".into())]);
        let mut previous_bottom = 0.0;
        let mut previous_collapsible = false;
        let mut diagnostic =
            super::super::super::super::text_measurement_diagnostic::TextMeasurementDiagnosticScope::enter(true);

        super::remember_margin_state(
            &[node],
            0,
            &CssStyle::browser_default(),
            false,
            &mut previous_bottom,
            &mut previous_collapsible,
        );

        assert_eq!(diagnostic.finish().style_calls, 1);
        assert_eq!((previous_bottom, previous_collapsible), (8.0, true));
    }

    #[test]
    fn margin_state_keeps_skip_and_reset_behavior() {
        assert_eq!(remembered_state("display:none", 5.0, true), (5.0, true));
        assert_eq!(remembered_state("display:contents", 5.0, true), (0.0, true));
        assert_eq!(
            remembered_state_for_node(HtmlDocumentNode::Text(" \n ".into()), 5.0, true),
            (5.0, true)
        );
        assert_eq!(
            remembered_state_for_node(HtmlDocumentNode::Text("content".into()), 5.0, true),
            (0.0, false)
        );
    }

    fn remembered_state(style: &str, bottom: f32, collapsible: bool) -> (f32, bool) {
        remembered_state_for_node(margin_node(style, Vec::new()), bottom, collapsible)
    }

    fn remembered_state_for_node(
        node: HtmlDocumentNode,
        mut bottom: f32,
        mut collapsible: bool,
    ) -> (f32, bool) {
        super::remember_margin_state(
            &[node],
            0,
            &CssStyle::browser_default(),
            false,
            &mut bottom,
            &mut collapsible,
        );
        (bottom, collapsible)
    }

    fn margin_node(style: &str, children: Vec<HtmlDocumentNode>) -> HtmlDocumentNode {
        HtmlDocumentNode::Element {
            node_id: TEST_NODE_ID,
            tag: "div".to_string(),
            attributes: vec![("style".to_string(), style.to_string())],
            children,
        }
    }

    #[test]
    fn adjacent_positive_block_margins_subtract_only_the_overlap() {
        assert_eq!(collapsed_margin_overlap(14.0, 16.0), 14.0);
        assert_eq!(collapsed_margin_overlap(16.0, 14.0), 14.0);
    }

    #[test]
    fn negative_margin_collapse_preserves_css_signed_sum() {
        assert_eq!(super::collapsed_margin(-8.0, 14.0), 6.0);
        assert_eq!(super::collapsed_margin(-8.0, -4.0), -8.0);
        assert_eq!(collapsed_margin_overlap(-8.0, -4.0), -4.0);
    }

    #[test]
    fn author_zero_margin_and_flex_grid_do_not_receive_block_collapse() -> Result<(), String> {
        assert_eq!(collapsed_margin_overlap(0.0, 0.0), 0.0);
        let inherited = CssStyle::browser_default();
        let flex = HtmlDocumentNode::Element {
            node_id: 1,
            tag: "div".to_string(),
            attributes: vec![(
                "style".to_string(),
                "display:flex;margin-top:16px".to_string(),
            )],
            children: Vec::new(),
        };
        assert!(super::block_margins(&flex, &inherited).is_some());
        assert!(!super::parent_allows_collapse(&CssStyle {
            display: taffy::prelude::Display::Flex,
            ..inherited.clone()
        }));
        let author_zero = HtmlDocumentNode::Element {
            node_id: 2,
            tag: "p".to_string(),
            attributes: vec![("style".to_string(), "margin:0".to_string())],
            children: Vec::new(),
        };
        let margins = super::block_margins(&author_zero, &inherited)
            .ok_or("author zero block margins missing")?;
        assert_eq!((margins.top, margins.bottom), (0.0, 0.0));
        Ok(())
    }

    #[test]
    fn rendered_block_siblings_match_chrome_heading_list_gap() -> Result<(), String> {
        let (heading, list) = render_heading_list_boxes()?;
        let gap = list.y - (heading.y + heading.height);
        assert!((gap - 16.0).abs() < 0.01, "heading/list gap was {gap}");
        Ok(())
    }

    fn render_heading_list_boxes() -> Result<
        (
            super::super::super::super::types::ElementBox,
            super::super::super::super::types::ElementBox,
        ),
        String,
    > {
        use super::super::super::super::types::DetailsContext;
        let mut renderer = heading_list_renderer()?;
        renderer.render_nodes(
            &heading_list_nodes(),
            0.0,
            0.0,
            576.0,
            &CssStyle::browser_default(),
            DetailsContext::NONE,
        );
        let heading = find_box(&renderer, 11, "heading")?;
        let list = find_box(&renderer, 12, "list")?;
        Ok((heading, list))
    }

    fn heading_list_renderer()
    -> Result<super::super::super::super::layout::HtmlLayoutRenderer, String> {
        use super::super::super::super::super::html_browser::HtmlBrowserViewport;
        use super::super::super::super::layout::HtmlLayoutRenderer;
        use std::collections::HashMap;

        HtmlBrowserViewport::new(620, 900, 1.0)
            .map_err(|error| format!("{error:?}"))
            .map(|viewport| HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None))
    }

    fn find_box(
        renderer: &super::super::super::super::layout::HtmlLayoutRenderer,
        node_id: u64,
        label: &str,
    ) -> Result<super::super::super::super::types::ElementBox, String> {
        renderer
            .element_boxes
            .iter()
            .find(|element| element.node_id == node_id)
            .cloned()
            .ok_or_else(|| format!("{label} box missing"))
    }

    fn heading_list_nodes() -> Vec<HtmlDocumentNode> {
        vec![
            HtmlDocumentNode::Element {
                node_id: 11,
                tag: "h2".to_string(),
                attributes: vec![(
                    "style".to_string(),
                    "margin-top:0;margin-bottom:14px".to_string(),
                )],
                children: vec![HtmlDocumentNode::Text("4. 利用手順".to_string())],
            },
            HtmlDocumentNode::Element {
                node_id: 12,
                tag: "ol".to_string(),
                attributes: vec![(
                    "style".to_string(),
                    "margin-top:16px;margin-bottom:0".to_string(),
                )],
                children: vec![HtmlDocumentNode::Element {
                    node_id: 13,
                    tag: "li".to_string(),
                    attributes: Vec::new(),
                    children: vec![HtmlDocumentNode::Text("項目".to_string())],
                }],
            },
        ]
    }
}
