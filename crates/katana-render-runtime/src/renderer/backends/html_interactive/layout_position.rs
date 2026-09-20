use super::constants::CONTROL_HEIGHT;
use super::layout::{ContainingBlock, HtmlLayoutRenderer};
use super::layout_input::is_checkbox;
use super::style::{CssPosition, CssStyle};
use super::types::{ElementPositioningContext, ElementRenderContext, LayoutContext};

#[path = "layout_position_ownership.rs"]
mod ownership;

fn horizontal_position(containing: ContainingBlock, style: &CssStyle, static_x: f32) -> (f32, f32) {
    let left = style.inset_left;
    let right = style.inset_right;
    let width = match (left, right) {
        (Some(left), Some(right)) => (containing.width - left - right).max(0.0),
        _ => style.box_width(containing.width).min(containing.width),
    };
    let x = left.map_or_else(
        || {
            right.map_or(static_x, |right| {
                containing.x + containing.width - right - width
            })
        },
        |left| containing.x + left,
    );
    (x, width)
}

fn vertical_position(
    containing: ContainingBlock,
    style: &CssStyle,
    static_y: f32,
) -> (f32, Option<f32>) {
    let top = style.inset_top;
    let bottom = style.inset_bottom;
    let explicit_height = style.height.map(|height| style.outer_height(height));
    let auto_count = auto_margin_count(style);
    let height = vertical_height(containing.height, top, bottom, explicit_height, auto_count);
    let auto_share = auto_vertical_share(containing.height, top, bottom, height, auto_count);
    let auto_top = top_auto_offset(style.margin_top_auto, auto_share);
    let y = top.map_or_else(
        || {
            bottom.zip(height).map_or(static_y, |(bottom, height)| {
                containing.y + containing.height - bottom - height - auto_top
            })
        },
        |top| containing.y + top + auto_top,
    );
    (y, height)
}

fn vertical_height(
    container_height: f32,
    top: Option<f32>,
    bottom: Option<f32>,
    explicit_height: Option<f32>,
    auto_count: usize,
) -> Option<f32> {
    match (top, bottom, explicit_height, auto_count == 0) {
        (Some(top), Some(bottom), None, true) => Some((container_height - top - bottom).max(0.0)),
        _ => explicit_height,
    }
}

fn auto_vertical_share(
    container_height: f32,
    top: Option<f32>,
    bottom: Option<f32>,
    height: Option<f32>,
    auto_count: usize,
) -> f32 {
    if auto_count == 0 {
        return 0.0;
    }
    let remaining = match (top, bottom, height) {
        (Some(top), Some(bottom), Some(height)) => {
            (container_height - top - bottom - height).max(0.0)
        }
        _ => 0.0,
    };
    remaining / auto_count as f32
}

fn top_auto_offset(margin_top_auto: bool, auto_share: f32) -> f32 {
    if margin_top_auto { auto_share } else { 0.0 }
}

fn auto_margin_count(style: &CssStyle) -> usize {
    usize::from(style.margin_top_auto) + usize::from(style.margin_bottom_auto)
}

impl HtmlLayoutRenderer {
    pub(super) fn render_positioned_or_flow_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
        style: &mut CssStyle,
    ) -> f32 {
        match style.position {
            CssPosition::Absolute | CssPosition::Fixed => {
                self.render_absolute_or_fixed_element(element, layout, style)
            }
            CssPosition::Sticky => self.render_sticky_element(element, layout, style),
            CssPosition::Static | CssPosition::Relative => {
                self.render_styled_element(element, LayoutContext { style, ..layout })
            }
        }
    }

    fn render_absolute_or_fixed_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
        style: &mut CssStyle,
    ) -> f32 {
        if element.tag == "input" && is_checkbox(element.attributes) && style.height.is_none() {
            let outer_height = style.explicit_width(layout.width).unwrap_or(CONTROL_HEIGHT);
            style.assign_outer_height(outer_height);
        }
        let (x, y, width) = self.positioned_geometry(style, (layout.x, layout.y));
        self.render_styled_element(
            element,
            LayoutContext::new(x, y, width, style, layout.details),
        );
        layout.y
    }

    pub(super) fn positioned_geometry(
        &self,
        style: &mut CssStyle,
        static_position: (f32, f32),
    ) -> (f32, f32, f32) {
        let containing = self.positioning_containing_block(style.position);
        let (x, width) = horizontal_position(containing, style, static_position.0);
        let (y, height) = vertical_position(containing, style, static_position.1);
        if let Some(height) = height {
            style.assign_outer_height(height);
        }
        style.width = None;
        style.max_width = None;
        (x, y, width)
    }

    pub(super) fn element_positioning_context(
        &self,
        position: CssPosition,
    ) -> ElementPositioningContext {
        match position {
            CssPosition::Fixed => ElementPositioningContext::FixedViewport,
            CssPosition::Absolute => ElementPositioningContext::AbsoluteContainingBlock {
                owner_node_id: self.positioning_containing_block(position).owner_node_id,
                viewport_escape: self.in_flow_positioning_context().escapes_element_root(),
            },
            CssPosition::Static | CssPosition::Relative | CssPosition::Sticky => {
                self.in_flow_positioning_context()
            }
        }
    }

    fn positioning_containing_block(&self, position: CssPosition) -> ContainingBlock {
        if position == CssPosition::Fixed {
            return ContainingBlock {
                owner_node_id: None,
                x: 0.0,
                y: self.scroll_y,
                width: self.viewport_width,
                height: self.viewport_height,
            };
        }
        self.ownership
            .containing_blocks
            .last()
            .copied()
            .unwrap_or(ContainingBlock {
                owner_node_id: None,
                x: 0.0,
                y: 0.0,
                width: self.viewport_width,
                height: self.viewport_height,
            })
    }

    pub(super) fn push_containing_block(&mut self, block: ContainingBlock) {
        self.ownership.containing_blocks.push(block);
    }

    pub(super) fn pop_containing_block(&mut self) {
        self.ownership.containing_blocks.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ContainingBlock, CssStyle, HtmlLayoutRenderer, auto_vertical_share, horizontal_position,
        vertical_position,
    };
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use std::collections::HashMap;

    fn containing_block() -> ContainingBlock {
        ContainingBlock {
            owner_node_id: None,
            x: 10.0,
            y: 20.0,
            width: 200.0,
            height: 100.0,
        }
    }

    #[test]
    fn positions_without_insets_preserve_the_static_flow_position() {
        let style = CssStyle::browser_default();
        assert_eq!(
            horizontal_position(containing_block(), &style, 35.0),
            (35.0, 200.0)
        );
        assert_eq!(
            vertical_position(containing_block(), &style, 47.0),
            (47.0, None)
        );
    }

    #[test]
    fn horizontal_position_honors_right_inset_with_explicit_width() {
        let mut style = CssStyle::browser_default();
        style.width = Some(super::super::style::CssLength::Px(50.0));
        style.inset_right = Some(15.0);

        assert_eq!(
            horizontal_position(containing_block(), &style, 35.0),
            (145.0, 50.0)
        );
    }

    #[test]
    fn vertical_auto_margins_center_an_explicit_height_between_insets() {
        let mut style = CssStyle::browser_default();
        style.inset_top = Some(0.0);
        style.inset_bottom = Some(0.0);
        style.height = Some(40.0);
        style.margin_top_auto = true;
        style.margin_bottom_auto = true;

        assert_eq!(
            vertical_position(containing_block(), &style, 47.0),
            (50.0, Some(40.0))
        );
    }

    #[test]
    fn vertical_auto_margin_has_no_share_without_complete_geometry() {
        assert_eq!(
            auto_vertical_share(100.0, Some(0.0), None, Some(40.0), 1),
            0.0
        );
    }

    #[test]
    fn container_stacks_are_updated_on_push_and_pop() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let block = containing_block();

        assert!(renderer.ownership.containing_blocks.is_empty());
        renderer.push_containing_block(block);
        assert_eq!(renderer.ownership.containing_blocks.len(), 1);
        assert_eq!(renderer.ownership.containing_blocks[0].x, block.x);
        assert_eq!(renderer.ownership.containing_blocks[0].y, block.y);
        assert_eq!(renderer.ownership.containing_blocks[0].width, block.width);
        assert_eq!(renderer.ownership.containing_blocks[0].height, block.height);

        renderer.pop_containing_block();
        assert!(renderer.ownership.containing_blocks.is_empty());
    }
}
