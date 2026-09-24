use super::super::layout::{ContainingBlock, HtmlLayoutRenderer};
use super::super::style::CssPosition;
use super::super::types::ElementPositioningContext;

impl HtmlLayoutRenderer {
    pub(in crate::renderer::backends::html_interactive) fn element_positioning_context(
        &self,
        position: CssPosition,
    ) -> ElementPositioningContext {
        match position {
            CssPosition::Fixed => {
                let containing = self.positioning_containing_block(position);
                containing.owner_node_id.map_or(
                    ElementPositioningContext::FixedViewport,
                    |owner_node_id| ElementPositioningContext::FixedContainingBlock {
                        owner_node_id,
                        viewport_escape: self.in_flow_positioning_context().escapes_element_root(),
                    },
                )
            }
            CssPosition::Absolute => ElementPositioningContext::AbsoluteContainingBlock {
                owner_node_id: self.positioning_containing_block(position).owner_node_id,
                viewport_escape: self.in_flow_positioning_context().escapes_element_root(),
            },
            CssPosition::Static | CssPosition::Relative | CssPosition::Sticky => {
                self.in_flow_positioning_context()
            }
        }
    }

    pub(in crate::renderer::backends::html_interactive) fn positioning_containing_block(
        &self,
        position: CssPosition,
    ) -> ContainingBlock {
        if position == CssPosition::Fixed {
            return self.fixed_positioning_containing_block();
        }
        self.ownership
            .containing_blocks
            .last()
            .copied()
            .unwrap_or_else(|| self.document_containing_block())
    }

    fn fixed_positioning_containing_block(&self) -> ContainingBlock {
        self.ownership
            .containing_blocks
            .iter()
            .rev()
            .find(|block| block.establishes_fixed_containing_block)
            .copied()
            .unwrap_or_else(|| self.viewport_containing_block())
    }

    fn document_containing_block(&self) -> ContainingBlock {
        self.default_containing_block(0.0)
    }

    fn viewport_containing_block(&self) -> ContainingBlock {
        self.default_containing_block(self.scroll_y)
    }

    fn default_containing_block(&self, y: f32) -> ContainingBlock {
        ContainingBlock {
            owner_node_id: None,
            x: 0.0,
            y,
            width: self.viewport_width,
            height: self.viewport_height,
            establishes_fixed_containing_block: false,
        }
    }
}
