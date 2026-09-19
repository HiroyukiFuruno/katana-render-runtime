use super::super::html_document::HtmlDocumentNode;
use super::document::attribute;
use super::layout::HtmlLayoutRenderer;
use super::layout_container::horizontal_box_geometry;
use super::style::{CssPosition, CssStyle};
use super::types::{
    DetailsContext, ElementBox, ElementRenderContext, HitTarget, HitTargetKind, LayoutContext,
};

#[path = "layout_element_box.rs"]
mod element_box;

impl HtmlLayoutRenderer {
    pub(super) fn render_node(
        &mut self,
        node: &HtmlDocumentNode,
        x: f32,
        y: f32,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> f32 {
        match node {
            HtmlDocumentNode::Text(text) => self.render_text(text, x, y, width, inherited),
            HtmlDocumentNode::Element {
                node_id,
                tag,
                attributes,
                children,
            } => self.render_element(
                ElementRenderContext {
                    node_id: *node_id,
                    tag,
                    attributes,
                    children,
                },
                LayoutContext::new(x, y, width, inherited, details),
            ),
        }
    }

    fn render_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let mut style = CssStyle::from_element(element.tag, element.attributes, layout.style);
        let paint_start = self.svg.len();
        let element_box_start = self.element_boxes.len();
        let records_inline_fragments = style.inline_block && !style.inline_atomic;
        self.ownership.rendering_elements.push(element.node_id);
        if records_inline_fragments {
            self.ownership.inline_fragment_owners.push(element.node_id);
        }
        let bottom = self.render_positioned_or_flow_element(element, layout, &mut style);
        if records_inline_fragments {
            debug_assert_eq!(
                self.ownership.inline_fragment_owners.pop(),
                Some(element.node_id)
            );
        }
        debug_assert_eq!(
            self.ownership.rendering_elements.pop(),
            Some(element.node_id)
        );
        self.finish_element_paint(paint_start, element_box_start, element.node_id, &style);
        bottom
    }

    fn finish_element_paint(
        &mut self,
        paint_start: usize,
        element_box_start: usize,
        node_id: u64,
        style: &CssStyle,
    ) {
        self.rotate_element_range(
            paint_start,
            element_box_start,
            node_id,
            style.rotation_degrees,
        );
        if style.opacity < 1.0 {
            self.wrap_painted_range(paint_start, style.opacity);
        }
        if style.position != CssPosition::Static
            && let Some(z_index) = style.z_index
        {
            self.defer_painted_range(paint_start, z_index);
        }
    }

    fn rotate_element_range(
        &mut self,
        paint_start: usize,
        element_box_start: usize,
        node_id: u64,
        rotation_degrees: f32,
    ) {
        if rotation_degrees == 0.0 {
            return;
        }
        let Some((center_x, center_y)) =
            ElementBox::center_after(&self.element_boxes, element_box_start, node_id)
        else {
            return;
        };
        self.wrap_rotated_range(
            paint_start,
            rotation_degrees,
            center_x,
            center_y - self.scroll_y,
        );
        for element_box in &mut self.element_boxes[element_box_start..] {
            element_box.rotate_about(rotation_degrees, center_x, center_y);
        }
    }
    pub(super) fn render_styled_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        if layout.style.display == taffy::style::Display::None {
            return layout.y;
        }
        let positioning_context = self.element_positioning_context(layout.style.position);
        let element_box_index = self.start_element_box(element.node_id, positioning_context);
        let target_index = self.start_click_target(element);
        self.record_anchor(element, layout.y);
        let bottom = self.render_tag(element, layout);
        self.finish_element_box(element_box_index, element.node_id, layout, bottom);
        if let Some(index) = target_index {
            self.finish_click_target(index, element.node_id, layout, bottom);
        }
        bottom
    }

    fn start_click_target(&mut self, element: ElementRenderContext<'_>) -> Option<usize> {
        let clickable = self.clickable_nodes.contains(&element.node_id)
            || attribute(element.attributes, "onclick").is_some();
        clickable.then(|| {
            let index = self.hit_targets.len();
            self.hit_targets.push(HitTarget {
                node_id: element.node_id,
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
                kind: HitTargetKind::Click,
            });
            index
        })
    }

    fn finish_click_target(
        &mut self,
        index: usize,
        node_id: u64,
        layout: LayoutContext<'_>,
        bottom: f32,
    ) {
        let (x, width) = horizontal_box_geometry(layout.x, layout.width, layout.style);
        let y = layout.y + layout.style.margin_top;
        self.hit_targets[index] = HitTarget {
            node_id,
            x,
            y,
            width,
            height: (bottom - y - layout.style.margin_bottom).max(0.0),
            kind: HitTargetKind::Click,
        };
    }

    fn record_anchor(&mut self, element: ElementRenderContext<'_>, y: f32) {
        let anchor = attribute(element.attributes, "id").or_else(|| {
            (element.tag == "a")
                .then(|| attribute(element.attributes, "name"))
                .flatten()
        });
        if let Some(anchor) = anchor.filter(|anchor| !anchor.is_empty()) {
            self.anchor_positions.entry(anchor.to_string()).or_insert(y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlLayoutRenderer;
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use crate::renderer::backends::html_document::HtmlDocumentNode;
    use crate::renderer::backends::html_interactive::types::{
        ElementBox, ElementPositioningContext, LayoutResult,
    };
    use std::collections::HashMap;

    const HALF_TURN_DEGREES: f32 = 180.0;
    const TEST_BOX_CENTER_X: f32 = 20.0;
    const TEST_BOX_CENTER_Y: f32 = 10.0;
    const TEST_BOX_WIDTH: f32 = 40.0;
    const TEST_BOX_HEIGHT: f32 = 20.0;

    #[test]
    fn ancestor_rotation_updates_descendant_element_boxes() {
        assert_eq!(
            render_rotated_ancestor()
                .as_ref()
                .map(descendant_axis_aligned),
            Ok(Some((60.0, -30.0, 10.0, 20.0)))
        );
    }

    #[test]
    fn element_boxes_keep_positioning_owner_and_overflow_clip_metadata() -> Result<(), String> {
        let layout = HtmlLayoutRenderer::render(
            &positioned_overflow_nodes(),
            HtmlBrowserViewport {
                width: 320,
                height: 240,
                device_scale_factor: 1.0,
            },
            0.0,
            &HashMap::new(),
            None,
        )?;
        let target = layout
            .element_boxes
            .iter()
            .find(|element| element.node_id == 2)
            .ok_or("target element box must exist")?;

        assert_eq!(
            target.positioning_context,
            ElementPositioningContext::AbsoluteContainingBlock {
                owner_node_id: Some(1),
            }
        );
        assert_eq!(target.overflow_clips.len(), 1);
        assert_eq!(target.overflow_clips[0].owner_node_id, 1);
        assert_eq!(target.overflow_clips[0].height, 30.0);
        Ok(())
    }

    #[test]
    fn absent_rotated_element_box_leaves_paint_unchanged() {
        let mut renderer = test_renderer();
        let paint_start = renderer.svg.len();
        renderer.rotate_element_range(paint_start, 0, 1, HALF_TURN_DEGREES);
        assert_eq!(renderer.svg.len(), paint_start);
    }

    #[test]
    fn half_turn_rotation_removes_sine_noise() {
        let mut element_box = test_element_box();
        element_box.rotate_about(HALF_TURN_DEGREES, TEST_BOX_CENTER_X, TEST_BOX_CENTER_Y);
        assert_eq!(
            element_box.transformed_corners,
            [
                (TEST_BOX_WIDTH, TEST_BOX_HEIGHT),
                (0.0, TEST_BOX_HEIGHT),
                (0.0, 0.0),
                (TEST_BOX_WIDTH, 0.0),
            ]
        );
    }

    fn test_renderer() -> HtmlLayoutRenderer {
        HtmlLayoutRenderer::new(
            HtmlBrowserViewport {
                width: 320,
                height: 240,
                device_scale_factor: 1.0,
            },
            0.0,
            &HashMap::new(),
            None,
        )
    }

    fn test_element_box() -> ElementBox {
        ElementBox {
            node_id: 1,
            x: 0.0,
            y: 0.0,
            width: TEST_BOX_WIDTH,
            height: TEST_BOX_HEIGHT,
            transformed_corners: ElementBox::rectangle_corners(
                0.0,
                0.0,
                TEST_BOX_WIDTH,
                TEST_BOX_HEIGHT,
            ),
            positioning_context: ElementPositioningContext::InFlow,
            overflow_clips: Vec::new(),
            inline_fragments: Vec::new(),
        }
    }

    fn render_rotated_ancestor() -> Result<LayoutResult, String> {
        HtmlLayoutRenderer::render(
            &rotated_ancestor_nodes(),
            HtmlBrowserViewport {
                width: 320,
                height: 240,
                device_scale_factor: 1.0,
            },
            0.0,
            &HashMap::new(),
            None,
        )
    }

    fn rotated_ancestor_nodes() -> Vec<HtmlDocumentNode> {
        vec![HtmlDocumentNode::Element {
            node_id: 1,
            tag: "div".to_string(),
            attributes: vec![(
                "style".to_string(),
                "width: 100px; height: 40px; transform: rotate(90deg)".to_string(),
            )],
            children: vec![HtmlDocumentNode::Element {
                node_id: 2,
                tag: "div".to_string(),
                attributes: vec![("style".to_string(), "width: 20px; height: 10px".to_string())],
                children: Vec::new(),
            }],
        }]
    }

    fn positioned_overflow_nodes() -> Vec<HtmlDocumentNode> {
        vec![HtmlDocumentNode::Element {
            node_id: 1,
            tag: "div".to_string(),
            attributes: vec![(
                "style".to_string(),
                "position: relative; width: 100px; height: 30px; overflow: hidden".to_string(),
            )],
            children: vec![HtmlDocumentNode::Element {
                node_id: 2,
                tag: "div".to_string(),
                attributes: vec![(
                    "style".to_string(),
                    "position: absolute; top: 0; width: 20px; height: 50px".to_string(),
                )],
                children: Vec::new(),
            }],
        }]
    }

    fn descendant_axis_aligned(layout: &LayoutResult) -> Option<(f32, f32, f32, f32)> {
        layout
            .element_boxes
            .iter()
            .find(|element| element.node_id == 2)
            .map(|target| target.transformed_axis_aligned())
    }
}
