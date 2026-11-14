use super::super::super::html_document::HtmlDocumentNode;
use super::super::layout::HtmlLayoutRenderer;
use super::super::style::CssStyle;
use super::super::types::ElementRenderContext;

impl HtmlLayoutRenderer {
    pub(super) fn enter_collapsible_margins(
        &mut self,
        element: ElementRenderContext<'_>,
        inherited: &CssStyle,
        style: &mut CssStyle,
    ) {
        let leading = if element.tag == "html"
            || matches!(
                inherited.display,
                taffy::prelude::Display::Flex | taffy::prelude::Display::Grid
            ) {
            0.0
        } else {
            super::super::layout_block_margin::leading_child_margin(element.children, style)
        };
        style.margin_top =
            super::super::layout_block_margin::collapsed_margin(style.margin_top, leading);
        let trailing = if element.tag == "html" {
            0.0
        } else {
            self.resolve_trailing_margin(element.children, inherited, style)
        };
        self.ownership.leading_child_margins.push(leading);
        self.ownership.trailing_child_margins.push(trailing);
        if matches!(element.tag, "html" | "body") {
            style.min_height = style
                .min_height
                .max((self.viewport_height - style.margin_top - style.margin_bottom).max(0.0));
        }
    }

    fn resolve_trailing_margin(
        &self,
        children: &[HtmlDocumentNode],
        inherited: &CssStyle,
        style: &mut CssStyle,
    ) -> f32 {
        if matches!(
            inherited.display,
            taffy::prelude::Display::Flex | taffy::prelude::Display::Grid
        ) {
            return 0.0;
        }
        let margin = super::super::layout_block_margin::trailing_child_margin(children, style);
        style.margin_bottom =
            super::super::layout_block_margin::collapsed_margin(style.margin_bottom, margin);
        margin
    }
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, HtmlLayoutRenderer};
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use crate::renderer::backends::html_document::HtmlDocument;
    use std::collections::HashMap;
    use taffy::prelude::Display;

    #[test]
    fn flex_and_grid_guard_preserves_trailing_margin() {
        let document =
            HtmlDocument::parse("<main><section><div>actual child</div></section></main>");
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 120,
            device_scale_factor: 1.0,
        };
        let renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        for display in [Display::Flex, Display::Grid] {
            let inherited = CssStyle {
                display,
                ..CssStyle::browser_default()
            };
            let mut style = CssStyle::browser_default();
            style.margin_bottom = 26.0;
            let trailing = renderer.resolve_trailing_margin(&nodes, &inherited, &mut style);
            assert_eq!(trailing, 0.0);
            assert_eq!(style.margin_bottom, 26.0);
        }
    }
}
