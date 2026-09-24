use super::super::layout::HtmlLayoutRenderer;
use super::super::types::{ElementBox, InlineFragment};

impl HtmlLayoutRenderer {
    pub(in crate::renderer::backends::html_interactive) fn finish_inline_fragment_box(
        &mut self,
        index: usize,
        node_id: u64,
        insertion_x: f32,
        insertion_y: f32,
    ) {
        let line_break_fragments = self.line_break_fragments(index, insertion_x);
        let element_box = &mut self.element_boxes[index];
        debug_assert_eq!(element_box.node_id, node_id);
        element_box.inline_fragments.extend(line_break_fragments);
        set_inline_fragment_bounds(element_box, insertion_x, insertion_y);
    }

    fn line_break_fragments(&self, index: usize, insertion_x: f32) -> Vec<InlineFragment> {
        self.element_boxes[index + 1..]
            .iter()
            .filter(|element_box| is_in_flow_fragment(element_box))
            .map(|element_box| {
                InlineFragment::new(insertion_x, element_box.y, 0.0, element_box.height)
            })
            .collect()
    }

    pub(in crate::renderer::backends::html_interactive) fn record_inline_fragment(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        if width < 0.0 || height <= 0.0 {
            return;
        }
        for owner in &self.ownership.inline_fragment_owners {
            if let Some(element_box) = self
                .element_boxes
                .iter_mut()
                .rev()
                .find(|element_box| element_box.node_id == *owner)
            {
                element_box
                    .inline_fragments
                    .push(InlineFragment::new(x, y, width, height));
            }
        }
    }
}

fn is_in_flow_fragment(element_box: &ElementBox) -> bool {
    element_box.participates_in_flow && element_box.width > 0.0 && element_box.height > 0.0
}

fn set_inline_fragment_bounds(element_box: &mut ElementBox, insertion_x: f32, insertion_y: f32) {
    let (x, y, width, height) = inline_fragment_bounds(&element_box.inline_fragments).unwrap_or((
        insertion_x,
        insertion_y,
        0.0,
        0.0,
    ));
    element_box.x = x;
    element_box.y = y;
    element_box.width = width;
    element_box.height = height;
    element_box.transformed_corners = ElementBox::rectangle_corners(x, y, width, height);
}

fn inline_fragment_bounds(fragments: &[InlineFragment]) -> Option<(f32, f32, f32, f32)> {
    let first = fragments.first()?;
    let (mut x, mut y, width, height) = first.transformed_axis_aligned();
    let (mut right, mut bottom) = (x + width, y + height);
    for fragment in fragments.iter().skip(1) {
        let (fragment_x, fragment_y, fragment_width, fragment_height) =
            fragment.transformed_axis_aligned();
        x = x.min(fragment_x);
        y = y.min(fragment_y);
        right = right.max(fragment_x + fragment_width);
        bottom = bottom.max(fragment_y + fragment_height);
    }
    Some((x, y, right - x, bottom - y))
}

#[cfg(test)]
mod tests {
    use super::super::super::layout::HtmlLayoutRenderer;
    use super::super::super::types::ElementPositioningContext;
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use std::collections::HashMap;

    #[test]
    fn record_inline_fragment_ignores_negative_width_and_zero_height() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        renderer.start_element_box(7, ElementPositioningContext::InFlow, true, None);
        renderer.ownership.inline_fragment_owners.push(7);

        renderer.record_inline_fragment(1.0, 2.0, -1.0, 3.0);
        renderer.record_inline_fragment(1.0, 2.0, 4.0, 0.0);

        assert!(renderer.element_boxes[0].inline_fragments.is_empty());
    }
}
