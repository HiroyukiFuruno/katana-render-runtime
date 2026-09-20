use super::{ELEMENT_BOX_CORNER_COUNT, ElementBox, OverflowClip};

const TRIGONOMETRIC_ZERO_EPSILON: f32 = 0.000_001;

#[derive(Debug, Clone)]
pub(crate) struct InlineFragment {
    pub(crate) transformed_corners: [(f32, f32); ELEMENT_BOX_CORNER_COUNT],
}

impl InlineFragment {
    pub(crate) fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            transformed_corners: ElementBox::rectangle_corners(x, y, width, height),
        }
    }

    pub(crate) fn rotate_about(&mut self, degrees: f32, center_x: f32, center_y: f32) {
        rotate_corners(&mut self.transformed_corners, degrees, center_x, center_y);
    }

    pub(crate) fn transformed_axis_aligned(&self) -> (f32, f32, f32, f32) {
        axis_aligned_bounds(&self.transformed_corners)
    }
}

impl ElementBox {
    pub(crate) fn rectangle_corners(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> [(f32, f32); ELEMENT_BOX_CORNER_COUNT] {
        [
            (x, y),
            (x + width, y),
            (x + width, y + height),
            (x, y + height),
        ]
    }

    pub(crate) fn center_after(
        boxes: &[Self],
        element_box_start: usize,
        node_id: u64,
    ) -> Option<(f32, f32)> {
        boxes
            .iter()
            .skip(element_box_start)
            .find(|element_box| element_box.node_id == node_id)
            .map(|element_box| {
                (
                    element_box.x + element_box.width / 2.0,
                    element_box.y + element_box.height / 2.0,
                )
            })
    }

    pub(crate) fn rotate_about(&mut self, degrees: f32, center_x: f32, center_y: f32) {
        rotate_corners(&mut self.transformed_corners, degrees, center_x, center_y);
        rotate_overflow_clip(&mut self.padding_edge, degrees, center_x, center_y);
        for overflow_clip in &mut self.overflow_clips {
            rotate_overflow_clip(overflow_clip, degrees, center_x, center_y);
        }
        for fragment in &mut self.inline_fragments {
            fragment.rotate_about(degrees, center_x, center_y);
        }
    }

    pub(crate) fn transformed_axis_aligned(&self) -> (f32, f32, f32, f32) {
        axis_aligned_bounds(&self.transformed_corners)
    }
}

fn rotate_corners(
    corners: &mut [(f32, f32); ELEMENT_BOX_CORNER_COUNT],
    degrees: f32,
    center_x: f32,
    center_y: f32,
) {
    let (mut sin, mut cos) = degrees.to_radians().sin_cos();
    if sin.abs() < TRIGONOMETRIC_ZERO_EPSILON {
        sin = 0.0;
    }
    if cos.abs() < TRIGONOMETRIC_ZERO_EPSILON {
        cos = 0.0;
    }
    for (x, y) in corners {
        let relative_x = *x - center_x;
        let relative_y = *y - center_y;
        *x = center_x + relative_x * cos - relative_y * sin;
        *y = center_y + relative_x * sin + relative_y * cos;
    }
}

fn rotate_overflow_clip(
    overflow_clip: &mut OverflowClip,
    degrees: f32,
    center_x: f32,
    center_y: f32,
) {
    rotate_corners(
        &mut overflow_clip.transformed_corners,
        degrees,
        center_x,
        center_y,
    );
    (
        overflow_clip.x,
        overflow_clip.y,
        overflow_clip.width,
        overflow_clip.height,
    ) = axis_aligned_bounds(&overflow_clip.transformed_corners);
}

fn axis_aligned_bounds(corners: &[(f32, f32); ELEMENT_BOX_CORNER_COUNT]) -> (f32, f32, f32, f32) {
    let (mut left, mut top) = corners[0];
    let (mut right, mut bottom) = (left, top);
    for (x, y) in corners.iter().skip(1) {
        left = left.min(*x);
        top = top.min(*y);
        right = right.max(*x);
        bottom = bottom.max(*y);
    }
    (left, top, right - left, bottom - top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::backends::html_interactive::types::ElementPositioningContext;

    #[test]
    fn rotating_element_box_rotates_overflow_clip_geometry() {
        let mut element_box = element_box_with_overflow_clip();

        element_box.rotate_about(90.0, 0.0, 0.0);

        assert_eq!(element_box.overflow_clips, vec![rotated_overflow_clip()]);
    }

    fn element_box_with_overflow_clip() -> ElementBox {
        ElementBox {
            node_id: 1,
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 20.0,
            transformed_corners: ElementBox::rectangle_corners(0.0, 0.0, 10.0, 20.0),
            positioning_context: ElementPositioningContext::InFlow,
            ancestor_node_ids: Vec::new(),
            overflow_clips: vec![OverflowClip::new(2, 10.0, 0.0, 20.0, 10.0, 0.0, 0.0)],
            clips_overflow: false,
            padding_edge: OverflowClip::new(1, 0.0, 0.0, 10.0, 20.0, 0.0, 0.0),
            inline_fragments: Vec::new(),
        }
    }

    fn rotated_overflow_clip() -> OverflowClip {
        let mut clip = OverflowClip::new(2, 10.0, 0.0, 20.0, 10.0, 0.0, 0.0);
        rotate_overflow_clip(&mut clip, 90.0, 0.0, 0.0);
        clip
    }
}
