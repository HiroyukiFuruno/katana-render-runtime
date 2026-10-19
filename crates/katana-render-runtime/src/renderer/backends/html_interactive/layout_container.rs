use super::super::html_browser::HtmlBrowserViewport;
use super::constants::BORDER_RADIUS_CORNER_COUNT;
use super::layout::{ContainingBlock, HtmlLayoutRenderer};
use super::style::{CssPosition, CssStyle};
use super::types::{DetailsContext, LayoutContext, OverflowClip};
use std::rc::Rc;

#[path = "layout_container_helpers.rs"]
mod helpers;
#[path = "layout_multicolumn.rs"]
mod multicolumn;
#[path = "layout_container_tests.rs"]
#[cfg(test)]
mod tests;

use super::super::html_document::HtmlDocumentNode;
use helpers::{
    ContainerGeometry, accept_flow_result, container_geometry, container_height,
    inline_container_width,
};

pub(super) fn horizontal_box_geometry(x: f32, width: f32, style: &CssStyle) -> (f32, f32) {
    helpers::horizontal_box_geometry(x, width, style)
}

impl HtmlLayoutRenderer {
    pub(super) fn render_container(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
        details: DetailsContext,
    ) -> f32 {
        self.render_container_with_list_markers(
            children,
            LayoutContext::new(x, y, width, style, details),
            None,
        )
    }

    pub(super) fn render_list_container(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
        ordered: bool,
    ) -> f32 {
        self.render_container_with_list_markers(children, layout, Some(ordered))
    }

    fn render_container_with_list_markers(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
        ordered_list: Option<bool>,
    ) -> f32 {
        let (x, y, width, style, details) = (
            layout.x,
            layout.y,
            layout.width,
            layout.style,
            layout.details,
        );
        let width = inline_container_width(children, width, style);
        let geometry = container_geometry(x, y, width, style);
        let box_start = self.svg.len();
        let descendant_box_start = self.element_boxes.len();
        let bottom =
            self.render_container_content(children, &geometry, style, details, ordered_list);
        let height = self.resolve_container_height(bottom, geometry.start, style);
        self.finish_container_paint(box_start, descendant_box_start, &geometry, height, style);
        geometry.start + height + style.margin_bottom
    }

    fn render_container_content(
        &mut self,
        children: &[HtmlDocumentNode],
        geometry: &ContainerGeometry,
        style: &CssStyle,
        details: DetailsContext,
        ordered_list: Option<bool>,
    ) -> f32 {
        let containing_block =
            self.resolve_container_containing_block(children, geometry, style, details);
        if let Some(block) = containing_block {
            self.push_containing_block(block);
        }
        let bottom =
            self.render_container_children(children, geometry, style, details, ordered_list);
        if containing_block.is_some() {
            self.pop_containing_block();
        }
        bottom
    }

    fn finish_container_paint(
        &mut self,
        box_start: usize,
        descendant_box_start: usize,
        geometry: &ContainerGeometry,
        height: f32,
        style: &CssStyle,
    ) {
        self.record_overflow_clip(descendant_box_start, geometry, height, style);
        self.paint_container_box(box_start, box_start, geometry, height, style);
    }

    fn resolve_container_height(&self, bottom: f32, start: f32, style: &CssStyle) -> f32 {
        let trailing_margin = self
            .ownership
            .trailing_child_margins
            .last()
            .copied()
            .unwrap_or(0.0);
        container_height(bottom - trailing_margin, start, style)
    }

    fn resolve_container_containing_block(
        &mut self,
        children: &[HtmlDocumentNode],
        geometry: &ContainerGeometry,
        style: &CssStyle,
        details: DetailsContext,
    ) -> Option<ContainingBlock> {
        if style.position == CssPosition::Static && !style.has_transform {
            return None;
        }
        let height = style
            .height
            .map(|height| style.outer_height(height))
            .unwrap_or_else(|| {
                let measured =
                    self.measure_auto_container_height(children, geometry, style, details);
                self.accept_auto_container_height(measured, style)
            });
        Some(ContainingBlock {
            owner_node_id: self.current_element_owner(),
            x: geometry.box_x,
            y: geometry.start,
            width: geometry.box_width,
            height,
            establishes_fixed_containing_block: style.has_transform,
        })
    }

    fn record_overflow_clip(
        &mut self,
        descendant_box_start: usize,
        geometry: &ContainerGeometry,
        height: f32,
        style: &CssStyle,
    ) {
        if !style.clips_overflow() {
            return;
        }
        let Some(owner_node_id) = self.current_element_owner() else {
            return;
        };
        if !self.clips_element_overflow(owner_node_id, style) {
            return;
        }
        let (x, y, width, height) = overflow_clip_geometry(geometry, height, style);
        let corner_radii = overflow_clip_radius(geometry, height, style);
        let clip = OverflowClip::new(owner_node_id, x, y, width, height, 0.0, 0.0)
            .with_active_axes(self.element_overflow_clip_axes(owner_node_id, style))
            .with_corner_radii(corner_radii);
        for element_box in &mut self.element_boxes[descendant_box_start..] {
            if overflow_clip_applies_to(element_box, owner_node_id) {
                element_box.overflow_clips.push(clip);
            }
        }
    }

    fn accept_auto_container_height(
        &mut self,
        measured: Result<f32, String>,
        style: &CssStyle,
    ) -> f32 {
        measured.unwrap_or_else(|error| {
            self.layout_error = Some(error);
            style.minimum_outer_height()
        })
    }

    fn measure_auto_container_height(
        &self,
        children: &[HtmlDocumentNode],
        geometry: &ContainerGeometry,
        style: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        let renderer = self.new_auto_measurement_renderer(geometry);
        let bottom = self
            .render_children_for_auto_measurement(renderer, children, geometry, style, details)?;
        Ok(container_height(bottom, 0.0, style))
    }

    fn new_auto_measurement_renderer(&self, geometry: &ContainerGeometry) -> Self {
        let viewport = HtmlBrowserViewport {
            width: geometry.inner_width.ceil().max(1.0) as u32,
            height: 1,
            device_scale_factor: 1.0,
        };
        Self::new_with_measurement_cache(
            viewport,
            0.0,
            &self.input_values,
            self.focused_input,
            Rc::clone(&self.flow_measurements),
        )
    }

    fn render_children_for_auto_measurement(
        &self,
        mut renderer: Self,
        children: &[HtmlDocumentNode],
        geometry: &ContainerGeometry,
        style: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        let mut child_style = style.clone();
        child_style.percentage_height_basis = None;
        let content_start = style.border_top_width() + style.padding_top;
        renderer
            .render_flow_children(
                children,
                LayoutContext::new(
                    0.0,
                    content_start,
                    geometry.inner_width,
                    &child_style,
                    details,
                ),
                None,
            )
            .and_then(|bottom| renderer.ensure_layout_succeeded().map(|()| bottom))
    }

    fn paint_container_box(
        &mut self,
        box_start: usize,
        content_start: usize,
        geometry: &ContainerGeometry,
        height: f32,
        style: &CssStyle,
    ) {
        self.paint_container_overflow(content_start, geometry, height, style);
        let before_background = self.svg.len();
        self.insert_box(
            box_start,
            geometry.box_x,
            geometry.start,
            geometry.box_width,
            height,
            style,
        );
        self.record_stacking_background(self.svg.len() - before_background);
    }

    fn paint_container_overflow(
        &mut self,
        content_start: usize,
        geometry: &ContainerGeometry,
        height: f32,
        style: &CssStyle,
    ) {
        self.capture_stacking_context_children();
        if style.clips_overflow()
            && self
                .current_element_owner()
                .is_none_or(|owner| self.clips_element_overflow(owner, style))
        {
            let source_owner = (geometry.box_x, geometry.start, geometry.box_width, height);
            let (x, y, width, height) = overflow_clip_geometry(geometry, height, style);
            let corner_radii = overflow_clip_radius(geometry, height, style);
            let Some(owner_node_id) = self.current_element_owner() else {
                self.layout_error = Some("overflow paint clip has no owner".to_string());
                return;
            };
            let clip = OverflowClip::new(owner_node_id, x, y, width, height, 0.0, 0.0)
                .with_active_axes(self.element_overflow_clip_axes(owner_node_id, style))
                .with_corner_radii(corner_radii);
            let clip_id = self.clip_painted_range(content_start, clip, source_owner);
            self.clip_deferred_paint(owner_node_id, clip_id);
        }
    }

    fn render_container_children(
        &mut self,
        children: &[HtmlDocumentNode],
        geometry: &ContainerGeometry,
        style: &CssStyle,
        details: DetailsContext,
        ordered_list: Option<bool>,
    ) -> f32 {
        let mut child_style = style.clone();
        let available_height = style.children_height();
        child_style.percentage_height_basis = available_height;
        let result = if ordered_list.is_none()
            && (style.column_count.is_some() || style.column_width.is_some())
        {
            self.render_multicolumn_children(children, geometry, style, details)
        } else {
            self.render_container_flow_children(
                children,
                geometry,
                &child_style,
                details,
                available_height,
                ordered_list,
            )
        };
        accept_flow_result(&mut self.layout_error, result, geometry.start)
    }

    fn render_container_flow_children(
        &mut self,
        children: &[HtmlDocumentNode],
        geometry: &ContainerGeometry,
        style: &CssStyle,
        details: DetailsContext,
        available_height: Option<f32>,
        ordered_list: Option<bool>,
    ) -> Result<f32, String> {
        let layout = LayoutContext::new(
            geometry.inner_x,
            self.container_content_start(geometry, style),
            geometry.inner_width,
            style,
            details,
        );
        match ordered_list {
            Some(ordered) => {
                self.render_list_flow_children(children, layout, available_height, ordered)
            }
            None => self.render_flow_children(children, layout, available_height),
        }
    }

    fn container_content_start(&self, geometry: &ContainerGeometry, style: &CssStyle) -> f32 {
        let leading_margin = self
            .ownership
            .leading_child_margins
            .last()
            .copied()
            .unwrap_or(0.0);
        geometry.start + style.border_top_width() + style.padding_top - leading_margin
    }
}

fn overflow_clip_geometry(
    geometry: &ContainerGeometry,
    height: f32,
    style: &CssStyle,
) -> (f32, f32, f32, f32) {
    let left = style.border_left_width();
    let top = style.border_top_width();
    let width = (geometry.box_width - left - style.border_right_width()).max(0.0);
    let height = (height - top - style.border_bottom_width()).max(0.0);
    (geometry.box_x + left, geometry.start + top, width, height)
}

fn overflow_clip_radius(
    geometry: &ContainerGeometry,
    height: f32,
    style: &CssStyle,
) -> [(f32, f32); BORDER_RADIUS_CORNER_COUNT] {
    style.resolved_inner_border_radii(geometry.box_width, height)
}

fn overflow_clip_applies_to(element_box: &super::types::ElementBox, owner_node_id: u64) -> bool {
    let context = element_box.positioning_context;
    if context.escapes_element_root() {
        return false;
    }
    if element_box.participates_in_flow {
        return true;
    }
    context
        .containing_block_owner()
        .is_some_and(|containing_block_owner| {
            contains_overflow_clip_owner(element_box, owner_node_id, containing_block_owner)
        })
}

fn contains_overflow_clip_owner(
    element_box: &super::types::ElementBox,
    owner_node_id: u64,
    containing_block_owner: u64,
) -> bool {
    element_box
        .ancestor_node_ids
        .iter()
        .position(|ancestor| *ancestor == containing_block_owner)
        .is_some_and(|containing_block_index| {
            element_box.ancestor_node_ids[..=containing_block_index].contains(&owner_node_id)
        })
}

#[cfg(test)]
mod container_contract_tests {
    use super::super::style::CssStyle;
    use super::super::types::DetailsContext;
    use super::{
        HtmlLayoutRenderer,
        helpers::{ContainerGeometry, container_geometry},
    };
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use crate::renderer::backends::html_document::HtmlDocumentNode;
    use crate::renderer::backends::html_interactive::types::{
        ElementBox, ElementPositioningContext, OverflowClip,
    };
    use std::collections::HashMap;

    fn geometry(width: f32, style: &CssStyle) -> ContainerGeometry {
        container_geometry(0.0, 0.0, width, style)
    }

    #[test]
    fn auto_container_height_uses_auto_measurement_when_height_is_not_explicit() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let children = [HtmlDocumentNode::Text("hello".to_string())];
        let mut style = CssStyle::browser_default();
        style.position = super::super::style::CssPosition::Absolute;
        style.height = None;
        let container = renderer.resolve_container_containing_block(
            &children,
            &geometry(300.0, &style),
            &style,
            DetailsContext::NONE,
        );

        assert!(container.is_some_and(|container| container.height > 0.0));
    }

    #[test]
    fn auto_measurement_measure_children_once_without_error() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let children = [HtmlDocumentNode::Text("measure".to_string())];
        let style = CssStyle::browser_default();
        let layout_geometry = geometry(300.0, &style);
        let measuring_renderer = renderer.new_auto_measurement_renderer(&layout_geometry);
        let bottom = renderer.render_children_for_auto_measurement(
            measuring_renderer,
            &children,
            &layout_geometry,
            &style,
            DetailsContext::NONE,
        );
        assert!(matches!(bottom, Ok(bottom) if bottom > 0.0));
    }

    #[test]
    fn failed_auto_height_measurement_records_error_and_uses_minimum_height() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let style = CssStyle::browser_default();

        let height =
            renderer.accept_auto_container_height(Err("measurement failed".to_string()), &style);

        assert_eq!(height, style.minimum_outer_height());
        assert_eq!(renderer.layout_error.as_deref(), Some("measurement failed"));
    }

    #[test]
    fn overflow_clip_and_positioning_helpers_fail_closed_without_an_owner() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let mut style = CssStyle::browser_default();
        style.overflow = super::super::style::CssOverflow::Clip;
        let geometry = geometry(300.0, &style);
        renderer.record_overflow_clip(0, &geometry, 20.0, &style);
        let element_box = absolute_box_without_owner();
        assert!(!super::overflow_clip_applies_to(&element_box, 1));
    }

    fn absolute_box_without_owner() -> ElementBox {
        ElementBox {
            node_id: 1,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            transformed_corners: [(0.0, 0.0); 4],
            positioning_context: ElementPositioningContext::AbsoluteContainingBlock {
                owner_node_id: None,
                viewport_escape: false,
            },
            positioning_origin_node_id: None,
            participates_in_flow: false,
            ancestor_node_ids: Vec::new(),
            overflow_clips: Vec::new(),
            overflow_clip_axes: (false, false),
            padding_edge: OverflowClip::new(1, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0),
            inline_fragments: Vec::new(),
        }
    }

    #[test]
    fn auto_measurement_propagates_existing_layout_error() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let style = CssStyle::browser_default();
        let layout_geometry = geometry(300.0, &style);
        let mut measuring_renderer = renderer.new_auto_measurement_renderer(&layout_geometry);
        measuring_renderer.layout_error = Some("existing failure".to_string());

        let result = renderer.render_children_for_auto_measurement(
            measuring_renderer,
            &[],
            &layout_geometry,
            &style,
            DetailsContext::NONE,
        );

        assert!(matches!(result, Err(error) if error == "existing failure"));
    }
}
