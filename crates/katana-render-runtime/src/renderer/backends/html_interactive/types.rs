use super::super::html_document::HtmlDocumentNode;
use super::style::CssStyle;

pub(super) const ELEMENT_BOX_CORNER_COUNT: usize = 4;

#[path = "types_geometry.rs"]
mod geometry;
#[path = "types_layout_result.rs"]
mod layout_result;
#[path = "types_positioning.rs"]
mod positioning;
pub(super) use geometry::InlineFragment;
pub(super) use layout_result::LayoutResult;
pub(super) use positioning::ElementPositioningContext;

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
    pub(super) previous_bottom_border: f32,
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
    /// element root の外へ出る配置文脈を開始した要素。
    pub(super) positioning_origin_node_id: Option<u64>,
    /// 自身が通常フローのレイアウトへ寄与するか。root 所有権とは分離する。
    pub(super) participates_in_flow: bool,
    /// 開始時点のDOM祖先。絶対配置要素へ適用する overflow clip を判定する。
    pub(super) ancestor_node_ids: Vec<u64>,
    pub(super) overflow_clips: Vec<OverflowClip>,
    /// 計算済みの両軸のoverflow clip。viewportへ伝播した要素では両方false。
    pub(super) overflow_clip_axes: (bool, bool),
    /// overflow clip を持つ IntersectionObserver の element root は padding edge を基準にする。
    pub(super) padding_edge: OverflowClip,
    pub(super) inline_fragments: Vec<InlineFragment>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct OverflowClip {
    pub(super) owner_node_id: u64,
    pub(super) active_axes: (bool, bool),
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    /// 回転後も clip の実形状を保つための四隅。x/y/width/height は AABB を保持する。
    pub(super) transformed_corners: [(f32, f32); ELEMENT_BOX_CORNER_COUNT],
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
            active_axes: (true, true),
            x,
            y,
            width,
            height,
            transformed_corners: ElementBox::rectangle_corners(x, y, width, height),
            corner_radii: [(radius_x, radius_y); ELEMENT_BOX_CORNER_COUNT],
        }
    }

    pub(super) fn with_active_axes(mut self, active_axes: (bool, bool)) -> Self {
        self.active_axes = active_axes;
        self
    }

    pub(super) fn with_corner_radii(
        mut self,
        corner_radii: [(f32, f32); ELEMENT_BOX_CORNER_COUNT],
    ) -> Self {
        self.corner_radii = corner_radii;
        self
    }

    pub(super) fn maximum_radii(&self) -> (f32, f32) {
        self.corner_radii
            .iter()
            .fold((0.0_f32, 0.0_f32), |radii, corner| {
                (radii.0.max(corner.0), radii.1.max(corner.1))
            })
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum HitTargetKind {
    Checkbox,
    Click,
    Input,
    Summary { details_node_id: u64 },
}
