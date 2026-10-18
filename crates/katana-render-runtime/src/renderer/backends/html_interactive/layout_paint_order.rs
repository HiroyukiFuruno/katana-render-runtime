use super::layout::{DeferredPaint, HtmlLayoutRenderer};
use super::style::{CssPosition, CssStyle};
#[path = "layout_paint_context.rs"]
mod context;
#[path = "layout_paint_fragment.rs"]
mod fragment;
use fragment::serialize_paint;
pub(super) use fragment::{PaintClip, PaintFragment, apply_deferred_clip};

impl HtmlLayoutRenderer {
    pub(super) fn reserve_auto_clip_paint_order(&mut self, style: &CssStyle) -> Option<u64> {
        if style.position == CssPosition::Static || style.z_index.is_some() {
            return None;
        }
        let clipped_lane = self.ownership.rendering_elements.iter().any(|owner| {
            self.ownership
                .element_box_indices
                .get(owner)
                .and_then(|index| self.element_boxes.get(*index))
                .is_some_and(|element| element.clips_overflow())
        });
        if !clipped_lane {
            return None;
        }
        /* WHY: 自動層は親背景を子より先に描くため、終了順ではなく入場順を予約する。 */
        let order = self.next_paint_order;
        self.next_paint_order += 1;
        Some(order)
    }

    pub(super) fn defer_auto_clip_paint(&mut self, start: usize, node_id: u64, order: Option<u64>) {
        let Some(order) = order else {
            return;
        };
        self.deferred_paint.push(DeferredPaint {
            node_id,
            z_index: 0,
            order,
            fragment: PaintFragment::Svg(self.svg.split_off(start)),
            clips: Vec::new(),
        });
    }

    pub(super) fn defer_paint_fragment(
        &mut self,
        fragment: PaintFragment,
        z_index: i32,
        node_id: u64,
    ) {
        let order = self.next_paint_order;
        self.next_paint_order += 1;
        self.deferred_paint.push(DeferredPaint {
            node_id,
            z_index,
            order,
            fragment,
            clips: Vec::new(),
        });
    }

    pub(super) fn wrap_painted_range(&mut self, start: usize, opacity: f32) {
        if self.svg.len() == start {
            return;
        }
        self.svg
            .insert_str(start, &format!(r#"<g opacity="{opacity}">"#));
        self.svg.push_str("</g>");
    }

    pub(super) fn wrap_rotated_range(
        &mut self,
        start: usize,
        degrees: f32,
        center_x: f32,
        center_y: f32,
    ) {
        if self.svg.len() == start {
            return;
        }
        self.svg.insert_str(
            start,
            &format!(r#"<g transform="rotate({degrees} {center_x} {center_y})">"#),
        );
        self.svg.push_str("</g>");
    }

    pub(super) fn finish_deferred_paint(&mut self) {
        self.deferred_paint
            .sort_by_key(|paint| (paint.z_index, paint.order));
        let mut negative = String::new();
        let mut foreground = String::new();
        for paint in self.deferred_paint.drain(..) {
            if paint.z_index < 0 {
                negative.push_str(&serialize_paint(paint, &[]));
            } else {
                foreground.push_str(&serialize_paint(paint, &[]));
            }
        }
        self.svg.insert_str(self.document_paint_start, &negative);
        self.svg.push_str(&foreground);
    }
}

#[cfg(test)]
#[path = "layout_paint_order_coverage_gap_tests.rs"]
mod coverage_gaps;

#[cfg(test)]
mod tests {
    use super::{HtmlLayoutRenderer, PaintFragment};
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use std::collections::HashMap;

    pub(super) fn renderer() -> HtmlLayoutRenderer {
        HtmlLayoutRenderer::new(
            HtmlBrowserViewport {
                width: 100,
                height: 100,
                device_scale_factor: 1.0,
            },
            0.0,
            &HashMap::new(),
            None,
        )
    }

    #[test]
    fn wrapping_empty_range_is_a_noop() {
        let mut renderer = renderer();
        let start = renderer.svg.len();

        renderer.wrap_painted_range(start, 0.5);

        assert_eq!(renderer.svg.len(), start);
    }

    #[test]
    fn rotated_range_uses_the_supplied_transform_origin() {
        let mut renderer = renderer();
        let start = renderer.svg.len();
        renderer.svg.push_str("content");

        renderer.wrap_rotated_range(start, 90.0, 20.0, 30.0);

        assert!(
            renderer
                .svg
                .contains(r#"<g transform="rotate(90 20 30)">content</g>"#)
        );
    }

    #[test]
    fn deferred_paint_places_negative_layers_behind_document() {
        let mut renderer = renderer();
        renderer.svg.push_str("document");
        let foreground = renderer.svg.len();
        renderer.svg.push_str("foreground");
        let foreground_svg = renderer.svg.split_off(foreground);
        renderer.defer_paint_fragment(PaintFragment::Svg(foreground_svg), 1, 1);
        let negative = renderer.svg.len();
        renderer.svg.push_str("negative");
        let negative_svg = renderer.svg.split_off(negative);
        renderer.defer_paint_fragment(PaintFragment::Svg(negative_svg), -1, 2);

        renderer.finish_deferred_paint();

        assert!(renderer.svg.ends_with("foreground"));
        assert!(renderer.svg.find("negative") < renderer.svg.find("document"));
    }

    #[test]
    fn rotating_an_empty_range_is_a_noop() {
        let mut renderer = renderer();
        let start = renderer.svg.len();

        renderer.wrap_rotated_range(start, 90.0, 0.0, 0.0);

        assert_eq!(renderer.svg.len(), start);
    }
}
