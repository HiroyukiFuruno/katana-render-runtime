use super::super::html_browser::HtmlBrowserViewport;
use super::constants::BORDER_RADIUS_CORNER_COUNT;
use super::layout::{ContainingBlock, HtmlLayoutRenderer};
use super::style::{CssPosition, CssStyle};
use super::types::{DetailsContext, LayoutContext, OverflowClip};
use std::rc::Rc;

#[path = "layout_container_measurement.rs"]
mod measurement;
#[path = "layout_container_paint.rs"]
mod paint;
#[cfg(test)]
use paint::overflow_clip_applies_to;

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
