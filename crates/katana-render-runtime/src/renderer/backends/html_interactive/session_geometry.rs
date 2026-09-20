use super::super::html_browser::{HtmlBrowserError, HtmlBrowserViewport};
use super::types::{ElementBox, ElementPositioningContext, HitTarget, LayoutResult};
use super::{HtmlInteractiveSession, runtime_failure};

impl HtmlInteractiveSession {
    pub(super) fn update_scroll(&mut self, render: &LayoutResult) {
        self.scroll_y = self.scroll_y.min(max_scroll_for(render, self.viewport));
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
            .update_layout_metrics_with_intersection_metadata(
                self.viewport.logical_width(),
                self.viewport.logical_height(),
                self.scroll_y,
                boxes.iter().map(|element| {
                    let (x, y, width, height) = element.transformed_axis_aligned();
                    (element.node_id, x, y, width, height, 0.0)
                }),
                boxes.iter().map(intersection_metadata),
            )
            .map_err(runtime_failure)
    }

    pub(super) fn hit_target_at(&self, x: f32, y: f32) -> Option<&HitTarget> {
        let scale = self.viewport.device_scale_factor;
        let x = x / scale;
        let document_y = y / scale + self.scroll_y;
        self.hit_targets
            .iter()
            .rev()
            .find(|target| contains(*target, x, document_y))
    }

    pub(super) fn element_at(&self, x: f32, y: f32) -> Option<&ElementBox> {
        let scale = self.viewport.device_scale_factor;
        let x = x / scale;
        let document_y = y / scale + self.scroll_y;
        self.element_boxes
            .iter()
            .rev()
            .find(|element| contains(*element, x, document_y))
    }

    pub(super) fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport.logical_height()).max(0.0)
    }
}

fn intersection_metadata(element: &ElementBox) -> (u64, String) {
    let (positioning, containing_block) = intersection_positioning(element);
    let clips = intersection_clips(element);
    let padding_edge = intersection_clip(&element.padding_edge);
    let fragments = intersection_fragments(element);
    (
        element.node_id,
        format!(
            "{{\"positioning\":\"{positioning}\",\"containingBlock\":{containing_block},\"paddingEdge\":{padding_edge},\"clips\":[{clips}],\"fragments\":[{fragments}]}}"
        ),
    )
}

fn intersection_positioning(element: &ElementBox) -> (&'static str, String) {
    match element.positioning_context {
        ElementPositioningContext::InFlow => ("in-flow", "null".to_string()),
        ElementPositioningContext::FixedViewport => ("fixed", "null".to_string()),
        ElementPositioningContext::AbsoluteContainingBlock { owner_node_id } => (
            "absolute",
            owner_node_id.map_or_else(|| "null".to_string(), |owner| owner.to_string()),
        ),
    }
}

fn intersection_clips(element: &ElementBox) -> String {
    element
        .overflow_clips
        .iter()
        .map(intersection_clip)
        .collect::<Vec<_>>()
        .join(",")
}

fn intersection_clip(clip: &super::types::OverflowClip) -> String {
    let corners = clip
        .transformed_corners
        .iter()
        .map(|(x, y)| format!("[{x},{y}]"))
        .collect::<Vec<_>>()
        .join(",");
    let radii = clip
        .corner_radii
        .iter()
        .map(|(radius_x, radius_y)| format!("[{radius_x},{radius_y}]"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"owner\":{},\"x\":{},\"y\":{},\"width\":{},\"height\":{},\"corners\":[{corners}],\"radiusX\":{},\"radiusY\":{},\"radii\":[{radii}]}}",
        clip.owner_node_id,
        clip.x,
        clip.y,
        clip.width,
        clip.height,
        clip.radius_x,
        clip.radius_y,
        radii = radii
    )
}

fn intersection_fragments(element: &ElementBox) -> String {
    element
        .inline_fragments
        .iter()
        .map(|fragment| {
            let (x, y, width, height) = fragment.transformed_axis_aligned();
            format!("{{\"x\":{x},\"y\":{y},\"width\":{width},\"height\":{height}}}")
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn max_scroll_for(render: &LayoutResult, viewport: HtmlBrowserViewport) -> f32 {
    (render.content_height - viewport.logical_height()).max(0.0)
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
