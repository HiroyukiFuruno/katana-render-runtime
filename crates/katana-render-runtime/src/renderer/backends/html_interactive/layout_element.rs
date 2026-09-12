use super::super::html_document::HtmlDocumentNode;
use super::document::attribute;
use super::layout::HtmlLayoutRenderer;
use super::layout_container::horizontal_box_geometry;
use super::style::{CssPosition, CssStyle};
use super::types::{
    DetailsContext, ElementBox, ElementRenderContext, HitTarget, HitTargetKind, LayoutContext,
};

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
        let bottom = self.render_positioned_or_flow_element(element, layout, &mut style);
        self.finish_element_paint(paint_start, element.node_id, &style);
        bottom
    }

    fn finish_element_paint(&mut self, paint_start: usize, node_id: u64, style: &CssStyle) {
        if style.rotation_degrees != 0.0
            && let Some(element_box) = self
                .element_boxes
                .iter()
                .rev()
                .find(|element_box| element_box.node_id == node_id)
        {
            self.wrap_rotated_range(
                paint_start,
                style.rotation_degrees,
                element_box.x + element_box.width / 2.0,
                element_box.y - self.scroll_y + element_box.height / 2.0,
            );
        }
        if style.opacity < 1.0 {
            self.wrap_painted_range(paint_start, style.opacity);
        }
        if style.position != CssPosition::Static
            && let Some(z_index) = style.z_index
        {
            self.defer_painted_range(paint_start, z_index);
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
        let element_box_index = self.start_element_box(element.node_id);
        let target_index = self.start_click_target(element);
        self.record_anchor(element, layout.y);
        let bottom = self.render_tag(element, layout);
        self.finish_element_box(element_box_index, element.node_id, layout, bottom);
        if let Some(index) = target_index {
            self.finish_click_target(index, element.node_id, layout, bottom);
        }
        bottom
    }

    fn start_element_box(&mut self, node_id: u64) -> usize {
        let index = self.element_boxes.len();
        self.element_boxes.push(ElementBox {
            node_id,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            rotation_degrees: 0.0,
        });
        index
    }

    fn finish_element_box(
        &mut self,
        index: usize,
        node_id: u64,
        layout: LayoutContext<'_>,
        bottom: f32,
    ) {
        let (x, width) = horizontal_box_geometry(layout.x, layout.width, layout.style);
        let y = layout.y + layout.style.margin_top;
        self.element_boxes[index] = ElementBox {
            node_id,
            x,
            y,
            width,
            height: (bottom - y - layout.style.margin_bottom).max(0.0),
            rotation_degrees: layout.style.rotation_degrees,
        };
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
