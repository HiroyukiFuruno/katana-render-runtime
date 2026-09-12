use super::super::html_document::HtmlDocumentNode;
use super::style::CssStyle;
use std::collections::HashMap;

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
    pub(super) rotation_degrees: f32,
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
