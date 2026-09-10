use super::HtmlDomBridgeState;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct HtmlLayoutMetrics {
    viewport_width: f32,
    viewport_height: f32,
    scroll_y: f32,
    boxes: HashMap<u64, HtmlLayoutBox>,
}

#[derive(Clone, Copy)]
struct HtmlLayoutBox {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl HtmlDomBridgeState {
    pub(crate) fn set_layout_metrics(
        &self,
        viewport_width: f32,
        viewport_height: f32,
        scroll_y: f32,
        boxes: impl IntoIterator<Item = (u64, f32, f32, f32, f32)>,
    ) {
        let boxes = boxes
            .into_iter()
            .map(|(node_id, x, y, width, height)| {
                (
                    node_id,
                    HtmlLayoutBox {
                        x,
                        y,
                        width,
                        height,
                    },
                )
            })
            .collect();
        *self.layout_metrics.borrow_mut() = HtmlLayoutMetrics {
            viewport_width,
            viewport_height,
            scroll_y,
            boxes,
        };
    }

    pub(super) fn layout_metrics_json(&self) -> String {
        let metrics = self.layout_metrics.borrow();
        format!(
            "{{\"width\":{},\"height\":{},\"scrollY\":{}}}",
            json_number(metrics.viewport_width),
            json_number(metrics.viewport_height),
            json_number(metrics.scroll_y),
        )
    }

    pub(super) fn bounding_client_rect_json(&self, node_id: u64) -> String {
        let metrics = self.layout_metrics.borrow();
        let Some(rect) = metrics.boxes.get(&node_id) else {
            return empty_rect_json();
        };
        let left = rect.x;
        let top = rect.y - metrics.scroll_y;
        let right = left + rect.width;
        let bottom = top + rect.height;
        format!(
            "{{\"x\":{},\"y\":{},\"width\":{},\"height\":{},\"top\":{},\"right\":{},\"bottom\":{},\"left\":{}}}",
            json_number(left),
            json_number(top),
            json_number(rect.width),
            json_number(rect.height),
            json_number(top),
            json_number(right),
            json_number(bottom),
            json_number(left),
        )
    }
}

fn empty_rect_json() -> String {
    "{\"x\":0,\"y\":0,\"width\":0,\"height\":0,\"top\":0,\"right\":0,\"bottom\":0,\"left\":0}"
        .to_string()
}

fn json_number(value: f32) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "0".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{HtmlDomBridgeState, json_number};
    use crate::renderer::backends::html_document::HtmlDocument;

    #[test]
    fn geometry_is_viewport_relative_and_uses_layout_boxes() {
        let state = HtmlDomBridgeState::new(HtmlDocument::parse("<p id=target>text</p>"));
        let target = state.document.borrow_mut().get_element_by_id("target");
        assert!(target.is_some());
        let mut node_ids = target.into_iter().collect::<Vec<_>>();
        let node_id = node_ids.remove(0);
        state.set_layout_metrics(320.0, 240.0, 80.0, [(node_id, 10.0, 100.0, 40.0, 20.0)]);

        assert_eq!(
            state.layout_metrics_json(),
            r#"{"width":320,"height":240,"scrollY":80}"#
        );
        assert_eq!(
            state.bounding_client_rect_json(node_id),
            r#"{"x":10,"y":20,"width":40,"height":20,"top":20,"right":50,"bottom":40,"left":10}"#
        );
    }

    #[test]
    fn non_finite_layout_numbers_are_serialized_as_zero() {
        assert_eq!(json_number(f32::NAN), "0");
    }
}
