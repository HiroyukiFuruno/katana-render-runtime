use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::types::{ElementRenderContext, LayoutContext};

impl HtmlLayoutRenderer {
    pub(super) fn render_sticky_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
        style: &CssStyle,
    ) -> f32 {
        let sticky_y = self.sticky_y(style, layout.y);
        let sticky_x = self.sticky_x(style, layout.x, layout.width);
        let bottom = self.render_styled_element(
            element,
            LayoutContext {
                x: sticky_x,
                y: sticky_y,
                style,
                ..layout
            },
        );
        layout.y + (bottom - sticky_y)
    }

    pub(super) fn sticky_y(&self, style: &CssStyle, static_y: f32) -> f32 {
        let height = style
            .height
            .map(|height| style.outer_height(height))
            .unwrap_or_else(|| style.minimum_outer_height());
        let mut y = static_y;
        if let Some(top) = style.inset_top {
            y = y.max(self.scroll_y + top);
        }
        if let Some(bottom) = style.inset_bottom {
            y = y.min(self.scroll_y + self.viewport_height - bottom - height);
        }
        if let Some(containing) = self.ownership.containing_blocks.last() {
            let minimum = containing.y;
            let maximum = (containing.y + containing.height - height).max(minimum);
            y = y.clamp(minimum, maximum);
        }
        y
    }

    pub(super) fn sticky_x(&self, style: &CssStyle, static_x: f32, available_width: f32) -> f32 {
        let width = style
            .explicit_width(available_width)
            .or_else(|| style.minimum_outer_width(available_width))
            .unwrap_or(0.0);
        let mut x = static_x;
        if let Some(left) = style.inset_left {
            x = x.max(self.scroll_x + left);
        }
        if let Some(right) = style.inset_right {
            x = x.min(self.scroll_x + self.viewport_width - right - width);
        }
        if let Some(containing) = self.ownership.containing_blocks.last() {
            let minimum = containing.x;
            let maximum = (containing.x + containing.width - width).max(minimum);
            x = x.clamp(minimum, maximum);
        }
        x
    }
}

#[cfg(test)]
mod tests {
    use super::super::layout::{ContainingBlock, HtmlLayoutRenderer};
    use super::super::style::{CssLength, CssStyle};
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use std::collections::HashMap;

    fn renderer(scroll_y: f32) -> HtmlLayoutRenderer {
        renderer_with_scroll(0.0, scroll_y)
    }

    fn renderer_with_scroll(scroll_x: f32, scroll_y: f32) -> HtmlLayoutRenderer {
        let mut renderer = HtmlLayoutRenderer::new(
            HtmlBrowserViewport {
                width: 320,
                height: 240,
                device_scale_factor: 1.0,
            },
            scroll_y,
            &HashMap::new(),
            None,
        );
        renderer.scroll_x = scroll_x;
        renderer
    }

    fn sticky_style() -> CssStyle {
        let mut style = CssStyle::browser_default();
        style.height = Some(100.0);
        style
    }

    #[test]
    fn sticky_top_tracks_the_scrollport_without_leaving_normal_flow() {
        let mut style = sticky_style();
        style.inset_top = Some(12.0);

        assert_eq!(renderer(300.0).sticky_y(&style, 40.0), 312.0);
    }

    #[test]
    fn sticky_top_stops_at_the_containing_block_bottom() {
        let mut renderer = renderer(500.0);
        renderer.push_containing_block(ContainingBlock {
            owner_node_id: None,
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 500.0,
            establishes_fixed_containing_block: false,
        });
        let mut style = sticky_style();
        style.inset_top = Some(0.0);

        assert_eq!(renderer.sticky_y(&style, 40.0), 400.0);
    }

    #[test]
    fn sticky_bottom_moves_an_element_inside_the_scrollport() {
        let mut style = sticky_style();
        style.inset_bottom = Some(10.0);

        assert_eq!(renderer(0.0).sticky_y(&style, 200.0), 130.0);
    }

    #[test]
    fn sticky_without_explicit_height_uses_the_minimum_outer_height() {
        let mut style = CssStyle::browser_default();
        style.inset_bottom = Some(10.0);
        style.min_height = 80.0;

        assert_eq!(renderer(0.0).sticky_y(&style, 200.0), 150.0);
    }

    #[test]
    fn sticky_left_and_right_follow_the_horizontal_scrollport() {
        let mut style = sticky_style();
        style.width = Some(CssLength::Px(100.0));
        style.inset_left = Some(12.0);
        assert_eq!(
            renderer_with_scroll(30.0, 0.0).sticky_x(&style, 0.0, 320.0),
            42.0
        );

        style.inset_left = None;
        style.inset_right = Some(10.0);
        assert_eq!(
            renderer_with_scroll(30.0, 0.0).sticky_x(&style, 300.0, 320.0),
            240.0
        );
    }

    #[test]
    fn sticky_x_resolves_percentage_width_and_containing_block_clamp() {
        let mut style = sticky_style();
        style.width = Some(CssLength::Percent(0.5));
        style.inset_right = Some(10.0);
        assert_eq!(
            renderer_with_scroll(30.0, 0.0).sticky_x(&style, 300.0, 160.0),
            260.0
        );

        style.width = Some(CssLength::Px(80.0));
        style.inset_right = None;
        let mut renderer = renderer_with_scroll(30.0, 0.0);
        renderer.push_containing_block(ContainingBlock {
            owner_node_id: None,
            x: 50.0,
            y: 0.0,
            width: 180.0,
            height: 240.0,
            establishes_fixed_containing_block: false,
        });
        assert_eq!(renderer.sticky_x(&style, 0.0, 320.0), 50.0);
        assert_eq!(renderer.sticky_x(&style, 120.0, 320.0), 120.0);
        assert_eq!(renderer.sticky_x(&style, 200.0, 320.0), 150.0);
    }
}
