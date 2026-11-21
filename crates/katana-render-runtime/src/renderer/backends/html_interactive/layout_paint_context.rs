use super::super::layout::{HtmlLayoutRenderer, StackingPaintContext};
use super::super::style::{CssPosition, CssStyle};
use super::super::types::ElementBox;
use super::PaintFragment;

impl HtmlLayoutRenderer {
    pub(in crate::renderer::backends::html_interactive) fn begin_stacking_paint_context(
        &mut self,
        node_id: u64,
        style: &CssStyle,
    ) -> Option<usize> {
        let position_creates_context =
            matches!(style.position, CssPosition::Fixed | CssPosition::Sticky);
        if !style.has_transform
            && style.opacity >= 1.0
            && !position_creates_context
            && (style.position == CssPosition::Static || style.z_index.is_none())
        {
            return None;
        }
        let start = self.deferred_paint.len();
        self.ownership.paint_contexts.push(StackingPaintContext {
            node_id,
            deferred_start: start,
            background_len: 0,
            children: Vec::new(),
        });
        Some(start)
    }

    pub(in crate::renderer::backends::html_interactive) fn capture_stacking_context_children(
        &mut self,
    ) {
        let owner = self.current_element_owner();
        let Some(context) = self.ownership.paint_contexts.last_mut() else {
            return;
        };
        if owner != Some(context.node_id) || context.deferred_start == self.deferred_paint.len() {
            return;
        }
        let paint = self.deferred_paint.split_off(context.deferred_start);
        context.children.extend(paint);
    }

    pub(in crate::renderer::backends::html_interactive) fn record_stacking_background(
        &mut self,
        length: usize,
    ) {
        let owner = self.current_element_owner();
        if let Some(context) = self.ownership.paint_contexts.last_mut()
            && owner == Some(context.node_id)
        {
            context.background_len = length;
        }
    }

    pub(in crate::renderer::backends::html_interactive) fn take_stacking_paint_context(
        &mut self,
        node_id: u64,
        deferred_start: Option<usize>,
    ) -> Option<StackingPaintContext> {
        let start = deferred_start?;
        let context = self.ownership.paint_contexts.pop();
        debug_assert_eq!(
            context
                .as_ref()
                .map(|item| (item.node_id, item.deferred_start)),
            Some((node_id, start))
        );
        context
    }

    pub(in crate::renderer::backends::html_interactive) fn finish_context_paint(
        &mut self,
        paint_start: usize,
        element_box_start: usize,
        node_id: u64,
        style: &CssStyle,
        mut context: StackingPaintContext,
    ) {
        context
            .children
            .sort_by_key(|paint| (paint.z_index, paint.order));
        let rotation =
            self.rotate_context_boxes(element_box_start, node_id, style.rotation_degrees);
        let mut background = self.svg.split_off(paint_start);
        let content = background.split_off(context.background_len);
        let fragment = PaintFragment::Context {
            background,
            content,
            children: context.children,
            rotation,
            opacity: style.opacity,
        };
        let z_index = if style.position == CssPosition::Static {
            0
        } else {
            style.z_index.unwrap_or(0)
        };
        self.defer_paint_fragment(fragment, z_index, node_id);
    }

    fn rotate_context_boxes(&mut self, start: usize, node_id: u64, degrees: f32) -> Option<String> {
        if degrees == 0.0 {
            return None;
        }
        let (x, y) = ElementBox::center_after(&self.element_boxes, start, node_id)?;
        for element_box in &mut self.element_boxes[start..] {
            element_box.rotate_about(degrees, x, y);
        }
        Some(format!(
            r#"<g transform="rotate({degrees} {} {})">"#,
            x - self.scroll_x,
            y - self.scroll_y
        ))
    }
}
