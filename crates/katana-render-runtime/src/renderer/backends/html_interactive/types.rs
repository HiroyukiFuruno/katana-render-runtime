use super::super::html_document::HtmlDocumentNode;
use super::style::CssStyle;
use std::collections::HashMap;

pub(super) const ELEMENT_BOX_CORNER_COUNT: usize = 4;

#[path = "types_geometry.rs"]
mod geometry;
pub(super) use geometry::InlineFragment;

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
    pub(super) positioning_context: ElementPositioningContext,
    /// 開始時点のDOM祖先。絶対配置要素へ適用する overflow clip を判定する。
    pub(super) ancestor_node_ids: Vec<u64>,
    pub(super) overflow_clips: Vec<OverflowClip>,
    /// 自身が overflow clip を持つか。element root の基準矩形に使う。
    pub(super) clips_overflow: bool,
    /// overflow clip を持つ IntersectionObserver の element root は padding edge を基準にする。
    pub(super) padding_edge: OverflowClip,
    pub(super) inline_fragments: Vec<InlineFragment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ElementPositioningContext {
    InFlow,
    FixedViewport,
    AbsoluteContainingBlock { owner_node_id: Option<u64> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct OverflowClip {
    pub(super) owner_node_id: u64,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    /// 回転後も clip の実形状を保つための四隅。x/y/width/height は AABB を保持する。
    pub(super) transformed_corners: [(f32, f32); ELEMENT_BOX_CORNER_COUNT],
    pub(super) radius_x: f32,
    pub(super) radius_y: f32,
    /// 内側の角丸半径を左上、右上、右下、左下の順で保持する。
    pub(super) corner_radii: [(f32, f32); ELEMENT_BOX_CORNER_COUNT],
}

impl OverflowClip {
    pub(super) fn new(
        owner_node_id: u64,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius_x: f32,
        radius_y: f32,
    ) -> Self {
        Self {
            owner_node_id,
            x,
            y,
            width,
            height,
            transformed_corners: ElementBox::rectangle_corners(x, y, width, height),
            radius_x,
            radius_y,
            corner_radii: [(radius_x, radius_y); ELEMENT_BOX_CORNER_COUNT],
        }
    }

    pub(super) fn with_corner_radii(
        mut self,
        corner_radii: [(f32, f32); ELEMENT_BOX_CORNER_COUNT],
    ) -> Self {
        self.corner_radii = corner_radii;
        self.radius_x = corner_radii
            .iter()
            .map(|(radius_x, _)| *radius_x)
            .fold(0.0, f32::max);
        self.radius_y = corner_radii
            .iter()
            .map(|(_, radius_y)| *radius_y)
            .fold(0.0, f32::max);
        self
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
