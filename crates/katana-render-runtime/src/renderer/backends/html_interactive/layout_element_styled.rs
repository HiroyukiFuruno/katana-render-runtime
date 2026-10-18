use super::{CssPosition, ElementRenderContext, HtmlLayoutRenderer, LayoutContext};

pub(super) fn render_styled_element(
    renderer: &mut HtmlLayoutRenderer,
    element: ElementRenderContext<'_>,
    layout: LayoutContext<'_>,
) -> f32 {
    if layout.style.display == taffy::style::Display::None {
        return layout.y;
    }
    let positioning_context = renderer.element_positioning_context(layout.style.position);
    let positioning_origin_node_id =
        renderer.positioning_origin_node_id(element.node_id, layout.style.position);
    let element_box_index = renderer.start_element_box(
        element.node_id,
        positioning_context,
        layout.style.position != CssPosition::Absolute
            && layout.style.position != CssPosition::Fixed,
        positioning_origin_node_id,
    );
    renderer.element_boxes[element_box_index].overflow_clip_axes =
        renderer.element_overflow_clip_axes(element.node_id, layout.style);
    let target_index = renderer.start_click_target(element);
    renderer.record_anchor(element, layout.y + layout.style.margin_top);
    let bottom = renderer.render_tag(element, layout);
    renderer.finish_element_box(element_box_index, element.node_id, layout, bottom);
    if let Some(index) = target_index {
        renderer.finish_click_target(index, element.node_id, layout, bottom);
    }
    bottom
}

#[cfg(test)]
mod tests {
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use crate::renderer::backends::html_document::HtmlDocument;
    use crate::renderer::backends::html_interactive::{
        layout::HtmlLayoutRenderer, types::LayoutResult,
    };
    use std::collections::HashMap;

    const ANCHOR_VIEWPORT_WIDTH: u32 = 320;
    const ANCHOR_VIEWPORT_HEIGHT: u32 = 240;
    const ANCHOR_VIEWPORT_SCALE: f32 = 1.0;
    const ANCHOR_SCROLL_Y: f32 = 0.0;

    #[test]
    fn anchors_match_signed_margin_boxes_and_preserve_first_duplicate_owner() -> Result<(), String>
    {
        let (layout, positive_id, negative_id, duplicate_id) = render_margin_fixture()?;
        let positive = element_box(&layout, positive_id)?;
        let negative = element_box(&layout, negative_id)?;
        let duplicate = element_box(&layout, duplicate_id)?;
        assert_eq!(layout.anchor_positions.get("positive"), Some(&positive.y));
        assert_eq!(layout.anchor_positions.get("negative"), Some(&negative.y));
        assert_eq!(layout.anchor_positions.get("duplicate"), Some(&duplicate.y));
        assert!(positive.y > 0.0, "positive margin did not move the box");
        assert!(
            negative.y < positive.y + positive.height,
            "negative margin did not reduce the block gap"
        );
        Ok(())
    }

    fn render_margin_fixture() -> Result<(LayoutResult, u64, u64, u64), String> {
        let html = "<style>html,body{margin:0}</style><main style='margin:0'><div id=positive style='margin-top:17px'>Positive</div><div id=negative style='margin-top:-7px'>Negative</div><div id=duplicate style='margin-top:11px;padding-top:1px'><div id=duplicate style='margin-top:13px'>Nested</div></div></main>";
        let mut document = HtmlDocument::parse(html);
        let positive_id = required_element_id(&mut document, "positive")?;
        let negative_id = required_element_id(&mut document, "negative")?;
        let duplicate_id = required_element_id(&mut document, "duplicate")?;
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let viewport = HtmlBrowserViewport {
            width: ANCHOR_VIEWPORT_WIDTH,
            height: ANCHOR_VIEWPORT_HEIGHT,
            device_scale_factor: ANCHOR_VIEWPORT_SCALE,
        };
        let layout =
            HtmlLayoutRenderer::render(&nodes, viewport, ANCHOR_SCROLL_Y, &HashMap::new(), None)?;
        Ok((layout, positive_id, negative_id, duplicate_id))
    }

    fn required_element_id(document: &mut HtmlDocument, id: &str) -> Result<u64, String> {
        document
            .get_element_by_id(id)
            .ok_or_else(|| format!("missing element {id}"))
    }

    fn element_box(
        layout: &LayoutResult,
        node_id: u64,
    ) -> Result<&crate::renderer::backends::html_interactive::types::ElementBox, String> {
        layout
            .element_boxes
            .iter()
            .find(|element_box| element_box.node_id == node_id)
            .ok_or_else(|| format!("missing element box for node {node_id}"))
    }
}
