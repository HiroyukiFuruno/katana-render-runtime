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
        let bottom = self.render_styled_element(
            element,
            LayoutContext {
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
}

#[cfg(test)]
mod tests {
    use super::super::layout::{ContainingBlock, HtmlLayoutRenderer};
    use super::super::style::CssStyle;
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use std::collections::HashMap;

    fn renderer(scroll_y: f32) -> HtmlLayoutRenderer {
        HtmlLayoutRenderer::new(
            HtmlBrowserViewport {
                width: 320,
                height: 240,
                device_scale_factor: 1.0,
            },
            scroll_y,
            &HashMap::new(),
            None,
        )
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
}
