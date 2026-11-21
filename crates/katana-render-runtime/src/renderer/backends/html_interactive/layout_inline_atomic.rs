use super::super::super::html_document::HtmlDocumentNode;
use super::super::constants::{INPUT_HEIGHT, LAYOUT_FLOAT_EPSILON};
use super::super::layout::HtmlLayoutRenderer;
use super::super::style::CssStyle;
use super::super::types::DetailsContext;
use super::render::whitespace::advance_pending_inline_space;
use super::state::InlineFlowState;

impl HtmlLayoutRenderer {
    pub(super) fn render_inline_node(
        &mut self,
        node: &HtmlDocumentNode,
        inline_width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
        inline: &mut InlineFlowState,
    ) {
        advance_pending_inline_space(inline, inherited);
        prepare_atomic_line(inline, inline_width);
        let element_box_start = self.element_boxes.len();
        let atomic_y = atomic_line_y(node, inherited, inline);
        inline.bottom = inline.bottom.max(self.render_node(
            node,
            inline.cursor_x,
            atomic_y,
            inline_width,
            inherited,
            details,
        ));
        self.record_inline_atomic_fragment(element_box_start);
        inline.cursor_x += inline_width;
        inline.has_items = true;
    }

    fn record_inline_atomic_fragment(&mut self, element_box_start: usize) {
        let Some(element_box) = self.element_boxes.get(element_box_start) else {
            return;
        };
        if !element_box.participates_in_flow || !element_box.inline_fragments.is_empty() {
            return;
        }
        let (x, y, width, height) = (
            element_box.x,
            element_box.y,
            element_box.width,
            element_box.height,
        );
        self.record_inline_fragment(x, y, width, height);
    }
}

fn prepare_atomic_line(inline: &mut InlineFlowState, width: f32) {
    if inline.has_items && inline.cursor_x + width > inline.x + inline.width + LAYOUT_FLOAT_EPSILON
    {
        inline.y = inline.bottom;
        inline.cursor_x = inline.x;
        inline.bottom = inline.y;
        inline.advance_line_index(1);
    }
}

fn atomic_line_y(node: &HtmlDocumentNode, inherited: &CssStyle, inline: &InlineFlowState) -> f32 {
    if let Some(height) = replaced_control_height(node, inherited) {
        return inline.y + (inherited.line_height - height).max(0.0);
    }
    super::atomic_metrics::inline_flex_metrics(node, inherited, inline.width).map_or(
        inline.y,
        |metrics| {
            let baseline = inline
                .text_line_style(0, inherited)
                .text_baseline_offset
                .unwrap_or(metrics.baseline);
            inline.y + (baseline - metrics.baseline).max(0.0)
        },
    )
}

fn replaced_control_height(node: &HtmlDocumentNode, inherited: &CssStyle) -> Option<f32> {
    let HtmlDocumentNode::Element {
        tag, attributes, ..
    } = node
    else {
        return None;
    };
    if tag != "input" {
        return None;
    }
    let style = CssStyle::from_element(tag, attributes, inherited);
    Some(
        style
            .height
            .map(|height| style.outer_height(height))
            .unwrap_or(INPUT_HEIGHT)
            .max(style.minimum_outer_height()),
    )
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, HtmlDocumentNode, replaced_control_height};

    #[test]
    fn input_replaced_control_uses_its_outer_height_for_baseline_alignment() {
        let node = HtmlDocumentNode::Element {
            node_id: 1,
            tag: "input".to_string(),
            attributes: Vec::new(),
            children: Vec::new(),
        };
        assert_eq!(
            replaced_control_height(&node, &CssStyle::browser_default()),
            Some(21.0)
        );
    }
}
