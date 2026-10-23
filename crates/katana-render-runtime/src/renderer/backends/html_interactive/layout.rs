#[path = "layout_initialization.rs"]
mod initialization;

use super::super::html_browser::HtmlBrowserViewport;
use super::layout_measurement_cache::FlowMeasurementCache;
use super::layout_paint_order::{PaintClip, PaintFragment};
use super::layout_viewport_overflow::HtmlViewportOverflowPolicy;
use super::types::{ElementBox, HitTarget, LayoutResult};
use crate::markdown::svg_rasterize::{GeneratedClip, GeneratedClipProvenance};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[derive(Default)]
pub(super) struct HtmlLayoutRenderer {
    pub(super) scroll_x: f32,
    pub(super) scroll_y: f32,
    pub(super) viewport_overflow: Option<HtmlViewportOverflowPolicy>,
    pub(super) svg: String,
    pub(super) hit_targets: Vec<HitTarget>,
    pub(super) element_boxes: Vec<ElementBox>,
    pub(super) anchor_positions: HashMap<String, f32>,
    pub(super) input_values: HashMap<u64, String>,
    pub(super) focused_input: Option<u64>,
    pub(super) layout_error: Option<String>,
    pub(super) viewport_height: f32,
    pub(super) viewport_width: f32,
    pub(super) next_clip_id: u64,
    pub(super) generated_clip_provenance: GeneratedClipProvenance,
    pub(super) next_gradient_id: u64,
    pub(super) ownership: LayoutOwnership,
    pub(super) clickable_nodes: std::collections::HashSet<u64>,
    pub(super) document_paint_start: usize,
    pub(super) deferred_paint: Vec<DeferredPaint>,
    pub(super) deferred_axis_clips: Vec<DeferredAxisClip>,
    pub(super) next_paint_order: u64,
    pub(super) flow_measurements: Rc<RefCell<FlowMeasurementCache>>,
    pub(super) font_markup_cache: super::layout_paint::paint_text::FontMarkupCache,
}

pub(super) struct DeferredPaint {
    pub(super) node_id: u64,
    pub(super) z_index: i32,
    pub(super) order: u64,
    pub(super) fragment: PaintFragment,
    pub(super) clips: Vec<PaintClip>,
}

pub(super) struct DeferredAxisClip {
    pub(super) id: u64,
    pub(super) owner_node_id: u64,
    pub(super) source_owner: (f32, f32, f32, f32),
    pub(super) clip: super::types::OverflowClip,
}

pub(super) struct StackingPaintContext {
    pub(super) node_id: u64,
    pub(super) deferred_start: usize,
    pub(super) background_len: usize,
    pub(super) children: Vec<DeferredPaint>,
}

#[derive(Clone, Copy)]
pub(super) struct ContainingBlock {
    pub(super) owner_node_id: Option<u64>,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) establishes_fixed_containing_block: bool,
}

#[derive(Default)]
pub(super) struct LayoutOwnership {
    pub(super) leading_child_margins: Vec<f32>,
    pub(super) trailing_child_margins: Vec<f32>,
    pub(super) containing_blocks: Vec<ContainingBlock>,
    pub(super) rendering_elements: Vec<u64>,
    pub(super) paint_contexts: Vec<StackingPaintContext>,
    pub(super) inline_fragment_owners: Vec<u64>,
    pub(super) element_box_indices: HashMap<u64, usize>,
}

impl HtmlLayoutRenderer {
    pub(super) fn record_generated_clip(&mut self, id: u64, shape: String) {
        self.generated_clip_provenance.clips.push(GeneratedClip {
            id: format!("krr-clip-{id}"),
            shape,
        });
    }
    pub(super) fn current_element_owner(&self) -> Option<u64> {
        self.ownership.rendering_elements.last().copied()
    }

    #[cfg(test)]
    pub(super) fn render(
        nodes: &[super::super::html_document::HtmlDocumentNode],
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
    ) -> Result<LayoutResult, String> {
        Self::render_with_clickable_nodes(
            nodes,
            viewport,
            scroll_y,
            input_values,
            focused_input,
            &std::collections::HashSet::new(),
        )
    }

    pub(super) fn into_layout_result(
        mut self,
        content_height: f32,
    ) -> Result<LayoutResult, String> {
        self.finalize_deferred_axis_clips()?;
        self.ensure_layout_succeeded()?;
        let (content_width, content_height) = super::layout_scroll_extent::scrollable_extent(
            &self.element_boxes,
            &self.ownership.element_box_indices,
            self.viewport_width,
            self.viewport_height,
            content_height,
        )?;
        Ok(self.finish_layout_result(content_width, content_height))
    }

    fn finish_layout_result(mut self, content_width: f32, content_height: f32) -> LayoutResult {
        let viewport_overflow = self.viewport_overflow.map_or(
            (
                super::super::html_browser::HtmlBrowserOverflowPolicy::Auto,
                super::super::html_browser::HtmlBrowserOverflowPolicy::Auto,
            ),
            |policy| policy.axes,
        );
        self.finish_deferred_paint();
        self.svg.push_str("</svg>");
        LayoutResult {
            svg: self.svg,
            generated_clip_provenance: self.generated_clip_provenance,
            hit_targets: self.hit_targets,
            element_boxes: self.element_boxes,
            anchor_positions: self.anchor_positions,
            content_width,
            content_height,
            viewport_overflow,
        }
    }

    pub(super) fn clips_element_overflow(
        &self,
        node_id: u64,
        style: &super::style::CssStyle,
    ) -> bool {
        let (x, y) = self.element_overflow_clip_axes(node_id, style);
        x || y
    }

    pub(super) fn element_overflow_clip_axes(
        &self,
        node_id: u64,
        style: &super::style::CssStyle,
    ) -> (bool, bool) {
        if self
            .viewport_overflow
            .and_then(|policy| policy.source_node_id)
            == Some(node_id)
        {
            return (false, false);
        }
        let (x, y) = style.computed_overflow_axes();
        (
            x != super::style::CssOverflowAxis::Visible,
            y != super::style::CssOverflowAxis::Visible,
        )
    }

    pub(super) fn ensure_layout_succeeded(&mut self) -> Result<(), String> {
        self.layout_error.take().map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::{HtmlBrowserViewport, HtmlLayoutRenderer};
    use std::collections::HashMap;

    #[test]
    fn renderer_propagates_recorded_layout_errors() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        renderer.layout_error = Some("layout failed".to_string());

        assert_eq!(
            renderer.ensure_layout_succeeded(),
            Err("layout failed".to_string())
        );
    }
}
