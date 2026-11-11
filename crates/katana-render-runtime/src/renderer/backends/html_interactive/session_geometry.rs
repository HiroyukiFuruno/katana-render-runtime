use super::super::html_browser::{HtmlBrowserError, HtmlBrowserViewport};
use super::layout_scroll_extent::clip_contains_point;
use super::types::{ElementBox, HitTarget, LayoutResult};
use super::{HtmlInteractiveSession, runtime_failure};

#[path = "session_geometry_intersection.rs"]
mod intersection;
#[path = "session_geometry_positioning.rs"]
mod positioning;

use intersection::intersection_metadata;

impl HtmlInteractiveSession {
    pub(super) fn update_scroll(&mut self, render: &LayoutResult) -> bool {
        let previous = (self.scroll_geometry.scroll_x, self.scroll_geometry.scroll_y);
        self.scroll_geometry.scroll_x = self.scroll_geometry.scroll_x.clamp(
            0.0,
            maximum_axis_scroll(render.content_width, self.viewport.logical_width()),
        );
        self.scroll_geometry.scroll_y = self
            .scroll_geometry
            .scroll_y
            .clamp(0.0, max_scroll_for(render, self.viewport));
        previous != (self.scroll_geometry.scroll_x, self.scroll_geometry.scroll_y)
    }

    pub(super) fn sync_intersection_geometry(
        &mut self,
        render: &LayoutResult,
    ) -> Result<bool, HtmlBrowserError> {
        self.sync_intersection_geometry_from_boxes(&render.element_boxes)
    }

    pub(super) fn sync_intersection_geometry_from_current_layout(
        &mut self,
    ) -> Result<bool, HtmlBrowserError> {
        let boxes = self.element_boxes.clone();
        self.sync_intersection_geometry_from_boxes(&boxes)
    }

    fn sync_intersection_geometry_from_boxes(
        &mut self,
        boxes: &[ElementBox],
    ) -> Result<bool, HtmlBrowserError> {
        self.runtime
            .update_layout_metrics_xy_with_intersection_metadata(
                self.viewport.logical_width(),
                self.viewport.logical_height(),
                (self.scroll_geometry.scroll_x, self.scroll_geometry.scroll_y),
                boxes.iter().map(|element| {
                    let (x, y, width, height) = element.transformed_axis_aligned();
                    (
                        (element.node_id, x, y, width, height, 0.0),
                        Some(intersection_metadata(element)),
                    )
                }),
            )
            .map_err(runtime_failure)
    }

    pub(super) fn hit_target_at(&self, x: f32, y: f32) -> Option<&HitTarget> {
        let scale = self.viewport.device_scale_factor;
        let x = x / scale + self.scroll_geometry.scroll_x;
        let document_y = y / scale + self.scroll_geometry.scroll_y;
        self.hit_targets.iter().rev().find(|target| {
            contains(*target, x, document_y)
                && self
                    .element_boxes
                    .iter()
                    .rev()
                    .find(|element| element.node_id == target.node_id)
                    .is_some_and(|element| visible_through_overflow_clips(element, x, document_y))
        })
    }

    pub(super) fn element_at(&self, x: f32, y: f32) -> Option<&ElementBox> {
        let scale = self.viewport.device_scale_factor;
        let x = x / scale + self.scroll_geometry.scroll_x;
        let document_y = y / scale + self.scroll_geometry.scroll_y;
        self.element_boxes.iter().rev().find(|element| {
            contains(*element, x, document_y)
                && visible_through_overflow_clips(element, x, document_y)
        })
    }

    pub(super) fn max_scroll(&self) -> f32 {
        maximum_document_scroll(self.scroll_geometry.content_height, self.viewport)
    }

    pub(super) fn max_scroll_x(&self) -> f32 {
        maximum_axis_scroll(
            self.scroll_geometry.content_width,
            self.viewport.logical_width(),
        )
    }
}

pub(super) fn max_scroll_for(render: &LayoutResult, viewport: HtmlBrowserViewport) -> f32 {
    maximum_document_scroll(render.content_height, viewport)
}

fn maximum_document_scroll(content_height: f32, viewport: HtmlBrowserViewport) -> f32 {
    /* WHY: ChromeのscrollHeightは整数CSS pixelへ丸められ、終端もその値を基準にする。 */
    maximum_axis_scroll(content_height, viewport.logical_height())
}

fn maximum_axis_scroll(content_extent: f32, viewport_extent: f32) -> f32 {
    (content_extent.round() - viewport_extent).max(0.0)
}

trait BoxGeometry {
    fn x(&self) -> f32;
    fn y(&self) -> f32;
    fn width(&self) -> f32;
    fn height(&self) -> f32;
}

impl BoxGeometry for HitTarget {
    fn x(&self) -> f32 {
        self.x
    }

    fn y(&self) -> f32 {
        self.y
    }

    fn width(&self) -> f32 {
        self.width
    }

    fn height(&self) -> f32 {
        self.height
    }
}

impl BoxGeometry for ElementBox {
    fn x(&self) -> f32 {
        self.x
    }

    fn y(&self) -> f32 {
        self.y
    }

    fn width(&self) -> f32 {
        self.width
    }

    fn height(&self) -> f32 {
        self.height
    }
}

fn contains(target: &impl BoxGeometry, x: f32, y: f32) -> bool {
    x >= target.x()
        && x <= target.x() + target.width()
        && y >= target.y()
        && y <= target.y() + target.height()
}

fn visible_through_overflow_clips(element: &ElementBox, x: f32, y: f32) -> bool {
    element
        .overflow_clips
        .iter()
        .all(|clip| clip_contains_point(clip, x, y))
}

#[cfg(test)]
mod document_scroll_tests {
    use super::{HtmlBrowserViewport, maximum_document_scroll};

    #[test]
    fn fractional_document_end_matches_chrome_scroll_height() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        for (height, expected) in [
            (300.125, 60.0),
            (300.484_38, 60.0),
            (300.5, 61.0),
            (300.75, 61.0),
            (300.984_38, 61.0),
            (200.0, 0.0),
        ] {
            assert_eq!(maximum_document_scroll(height, viewport), expected);
        }
    }
}

#[cfg(test)]
mod overflow_clip_hit_tests {
    use super::visible_through_overflow_clips;
    use crate::renderer::backends::html_interactive::types::{
        ElementBox, ElementPositioningContext, OverflowClip,
    };

    fn element_with_clip(clip: Option<OverflowClip>) -> ElementBox {
        let x = 0.0;
        let y = 0.0;
        let width = 100.0;
        let height = 100.0;
        ElementBox {
            node_id: 1,
            x,
            y,
            width,
            height,
            transformed_corners: ElementBox::rectangle_corners(x, y, width, height),
            positioning_context: ElementPositioningContext::InFlow,
            positioning_origin_node_id: None,
            participates_in_flow: true,
            ancestor_node_ids: Vec::new(),
            overflow_clips: clip.into_iter().collect(),
            overflow_clip_axes: (true, true),
            padding_edge: OverflowClip::new(1, x, y, width, height, 0.0, 0.0),
            inline_fragments: Vec::new(),
        }
    }

    #[test]
    fn no_clip_keeps_hit_inside_element() {
        let element = element_with_clip(None);
        assert!(visible_through_overflow_clips(&element, 90.0, 90.0));
    }

    #[test]
    fn hit_target_without_owner_element_is_rejected() {
        let owner: Option<&ElementBox> = None;
        assert!(
            !owner.is_some_and(|element| { visible_through_overflow_clips(element, 20.0, 20.0) })
        );
    }

    #[test]
    fn inactive_axes_do_not_reject_point_outside_clip_bounds() {
        let clip =
            OverflowClip::new(2, 10.0, 10.0, 40.0, 40.0, 0.0, 0.0).with_active_axes((false, false));
        let element = element_with_clip(Some(clip));
        assert!(visible_through_overflow_clips(&element, 90.0, 90.0));
    }

    #[test]
    fn active_x_and_y_axes_reject_only_their_own_outside_points() {
        let x_clip =
            OverflowClip::new(2, 10.0, 10.0, 40.0, 40.0, 0.0, 0.0).with_active_axes((true, false));
        let y_clip =
            OverflowClip::new(3, 10.0, 10.0, 40.0, 40.0, 0.0, 0.0).with_active_axes((false, true));
        assert!(!visible_through_overflow_clips(
            &element_with_clip(Some(x_clip)),
            5.0,
            20.0
        ));
        assert!(visible_through_overflow_clips(
            &element_with_clip(Some(x_clip)),
            20.0,
            90.0
        ));
        assert!(!visible_through_overflow_clips(
            &element_with_clip(Some(y_clip)),
            20.0,
            5.0
        ));
        assert!(visible_through_overflow_clips(
            &element_with_clip(Some(y_clip)),
            90.0,
            20.0
        ));
    }

    #[test]
    fn rotated_clip_uses_transformed_polygon() {
        let mut element =
            element_with_clip(Some(OverflowClip::new(2, 25.0, 25.0, 50.0, 50.0, 0.0, 0.0)));
        element.rotate_about(45.0, 50.0, 50.0);
        assert!(visible_through_overflow_clips(&element, 50.0, 50.0));
        assert!(!visible_through_overflow_clips(&element, 25.0, 25.0));
    }
}
