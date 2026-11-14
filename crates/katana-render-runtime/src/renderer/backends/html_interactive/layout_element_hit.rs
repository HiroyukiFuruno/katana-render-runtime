use super::super::document::attribute;
use super::super::layout::HtmlLayoutRenderer;
use super::super::layout_container::horizontal_box_geometry;
use super::super::types::{ElementRenderContext, HitTarget, HitTargetKind, LayoutContext};

impl HtmlLayoutRenderer {
    pub(super) fn start_click_target(
        &mut self,
        element: ElementRenderContext<'_>,
    ) -> Option<usize> {
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

    pub(super) fn finish_click_target(
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

    pub(super) fn record_anchor(&mut self, element: ElementRenderContext<'_>, y: f32) {
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
