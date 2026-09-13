use super::super::html_document::HtmlDocumentNode;
use super::style::CssStyle;
use std::collections::HashMap;

const TRIGONOMETRIC_ZERO_EPSILON: f32 = 0.000_001;
pub(super) const ELEMENT_BOX_CORNER_COUNT: usize = 4;

#[derive(Clone, Copy)]
pub(super) struct ElementRenderContext<'a> {
    pub(super) node_id: u64,
    pub(super) tag: &'a str,
    pub(super) attributes: &'a [(String, String)],
    pub(super) children: &'a [HtmlDocumentNode],
}

#[derive(Clone, Copy)]
pub(super) struct LayoutContext<'a> {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) style: &'a CssStyle,
    pub(super) details: DetailsContext,
}

impl<'a> LayoutContext<'a> {
    pub(super) fn new(
        x: f32,
        y: f32,
        width: f32,
        style: &'a CssStyle,
        details: DetailsContext,
    ) -> Self {
        Self {
            x,
            y,
            width,
            style,
            details,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct DetailsContext {
    pub(super) node_id: Option<u64>,
    pub(super) open: bool,
}

impl DetailsContext {
    pub(super) const NONE: Self = Self {
        node_id: None,
        open: false,
    };

    pub(super) fn from_open_state(node_id: u64, open: bool) -> Self {
        Self {
            node_id: Some(node_id),
            open,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ControlLayout<'a> {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) style: &'a CssStyle,
}

#[derive(Clone, Copy)]
pub(super) struct TableCellLayout<'a> {
    pub(super) row_index: usize,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) style: &'a CssStyle,
}

#[derive(Debug, Clone)]
pub(super) struct HitTarget {
    pub(super) node_id: u64,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) kind: HitTargetKind,
}

#[derive(Debug, Clone)]
pub(super) struct ElementBox {
    pub(super) node_id: u64,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) transformed_corners: [(f32, f32); ELEMENT_BOX_CORNER_COUNT],
}

pub(super) fn rectangle_corners(
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

pub(super) fn element_box_center_after(
    boxes: &[ElementBox],
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

impl ElementBox {
    pub(super) fn rotate_about(&mut self, degrees: f32, center_x: f32, center_y: f32) {
        let (mut sin, mut cos) = degrees.to_radians().sin_cos();
        if sin.abs() < TRIGONOMETRIC_ZERO_EPSILON {
            sin = 0.0;
        }
        if cos.abs() < TRIGONOMETRIC_ZERO_EPSILON {
            cos = 0.0;
        }
        for (x, y) in &mut self.transformed_corners {
            let relative_x = *x - center_x;
            let relative_y = *y - center_y;
            *x = center_x + relative_x * cos - relative_y * sin;
            *y = center_y + relative_x * sin + relative_y * cos;
        }
    }

    pub(super) fn transformed_axis_aligned(&self) -> (f32, f32, f32, f32) {
        let (mut left, mut top) = self.transformed_corners[0];
        let (mut right, mut bottom) = (left, top);
        for (x, y) in self.transformed_corners.iter().skip(1) {
            left = left.min(*x);
            top = top.min(*y);
            right = right.max(*x);
            bottom = bottom.max(*y);
        }
        (left, top, right - left, bottom - top)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum HitTargetKind {
    Checkbox,
    Click,
    Input,
    Summary { details_node_id: u64 },
}

pub(super) struct LayoutResult {
    pub(super) svg: String,
    pub(super) hit_targets: Vec<HitTarget>,
    pub(super) element_boxes: Vec<ElementBox>,
    pub(super) anchor_positions: HashMap<String, f32>,
    pub(super) content_height: f32,
}
