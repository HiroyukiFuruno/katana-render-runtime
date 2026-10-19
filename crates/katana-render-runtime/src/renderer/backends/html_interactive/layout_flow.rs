use super::super::html_document::HtmlDocumentNode;
use super::layout::HtmlLayoutRenderer;
use super::types::LayoutContext;
use taffy::prelude::{Display, TaffyTree};

#[path = "layout_flow_compute.rs"]
mod compute;
#[path = "layout_flow_list.rs"]
mod list;

use compute::{compute_flow_layout, compute_root_layout};
pub(super) use list::ListItemMarker;
use list::flow_items;

impl HtmlLayoutRenderer {
    pub(super) fn render_flow_children(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
        available_height: Option<f32>,
    ) -> Result<f32, String> {
        match layout.style.display {
            Display::Flex | Display::Grid => {
                self.render_taffy_flow(children, layout, available_height, None)
            }
            Display::Block | Display::FlowRoot => Ok(self.render_nodes(
                children,
                layout.x,
                layout.y,
                layout.width,
                layout.style,
                layout.details,
            )),
            Display::None => Ok(layout.y),
        }
    }

    fn render_taffy_flow(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
        available_height: Option<f32>,
        list_markers: Option<bool>,
    ) -> Result<f32, String> {
        let (y, width) = (layout.y, layout.width);
        let (style, details) = (layout.style, layout.details);
        let items = flow_items(children, style, list_markers.is_some());
        if items.is_empty() {
            return Ok(y);
        }
        let mut tree = TaffyTree::<()>::new();
        tree.disable_rounding();
        let list_mode = list_markers.is_some();
        let nodes = self.build_flow_nodes(&mut tree, &items, width, style, details, list_mode)?;
        let root = compute_flow_layout(&mut tree, &nodes, style, width, available_height)?;
        self.remeasure_flow_heights(&mut tree, &nodes, &items, layout, list_mode)?;
        compute_root_layout(&mut tree, root, width, available_height)?;
        self.paint_flow_items(&tree, &nodes, &items, layout, list_markers)
    }
}

#[cfg(test)]
mod tests {
    use super::super::layout::HtmlLayoutRenderer;
    use super::super::style::CssStyle;
    use super::super::types::{DetailsContext, LayoutContext};
    use super::Display;
    use super::compute::layout_error;
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use crate::renderer::backends::html_document::HtmlDocumentNode;
    use std::collections::HashMap;

    #[test]
    fn none_and_empty_flow_paths_preserve_the_current_position() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let mut style = CssStyle::browser_default();
        style.display = Display::None;
        assert_empty_flow_position(&mut renderer, &style, 7.0);

        style.display = Display::FlowRoot;
        assert_empty_flow_position(&mut renderer, &style, 8.0);

        style.display = Display::Flex;
        let whitespace = [HtmlDocumentNode::Text(" \n ".to_string())];
        assert_eq!(
            renderer.render_flow_children(
                &whitespace,
                LayoutContext::new(0.0, 9.0, 100.0, &style, DetailsContext::NONE),
                None,
            ),
            Ok(9.0)
        );
        assert_eq!(layout_error("boom"), "CSS flow layout failed: boom");
    }

    fn assert_empty_flow_position(renderer: &mut HtmlLayoutRenderer, style: &CssStyle, y: f32) {
        assert_eq!(
            renderer.render_flow_children(
                &[],
                LayoutContext::new(0.0, y, 100.0, style, DetailsContext::NONE),
                None,
            ),
            Ok(y)
        );
    }
}
