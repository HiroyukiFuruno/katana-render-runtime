use super::super::super::html_document::HtmlDocumentNode;
use super::super::constants::MIN_LAYOUT_WIDTH;
use super::super::layout_inline::InlineMeasurement;
use super::super::style::CssStyle;

pub(super) struct ContainerGeometry {
    pub(super) start: f32,
    pub(super) box_x: f32,
    pub(super) box_width: f32,
    pub(super) inner_x: f32,
    pub(super) inner_width: f32,
}

pub(super) fn inline_container_width(
    children: &[HtmlDocumentNode],
    available: f32,
    style: &CssStyle,
) -> f32 {
    if !style.inline_block || style.width.is_some() || style.max_width.is_some() {
        return available;
    }
    (InlineMeasurement::content_box_width(children, style, available)
        + style.margin_left
        + style.margin_right)
        .min(available)
        .max(MIN_LAYOUT_WIDTH)
}

pub(super) fn container_geometry(
    x: f32,
    y: f32,
    width: f32,
    style: &CssStyle,
) -> ContainerGeometry {
    let start = y + style.margin_top;
    let (box_x, box_width) = horizontal_box_geometry(x, width, style);
    ContainerGeometry {
        start,
        box_x,
        box_width,
        inner_x: box_x + style.border_left_width() + style.padding_left,
        inner_width: style.content_width(box_width).max(MIN_LAYOUT_WIDTH),
    }
}

pub(super) fn horizontal_box_geometry(x: f32, width: f32, style: &CssStyle) -> (f32, f32) {
    let available_width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
    let box_width = style.box_width(available_width).max(MIN_LAYOUT_WIDTH);
    let remaining = (available_width - box_width).max(0.0);
    let auto_count = usize::from(style.margin_left_auto) + usize::from(style.margin_right_auto);
    let auto_share = if auto_count == 0 {
        0.0
    } else {
        remaining / auto_count as f32
    };
    let auto_left = if style.margin_left_auto {
        auto_share
    } else {
        0.0
    };
    (x + style.margin_left + auto_left, box_width)
}

pub(super) fn container_height(bottom: f32, start: f32, style: &CssStyle) -> f32 {
    let natural = bottom - start + style.padding_bottom + style.border_bottom_width();
    let resolved = style.height.map_or_else(
        || natural.max(style.minimum_outer_height()),
        |height| style.outer_height(height).max(style.minimum_outer_height()),
    );
    style.max_height.map_or(resolved, |maximum| {
        resolved.min(style.outer_height(maximum))
    })
}

pub(super) fn accept_flow_result(
    layout_error: &mut Option<String>,
    result: Result<f32, String>,
    start: f32,
) -> f32 {
    match result {
        Ok(bottom) => bottom,
        Err(error) => {
            *layout_error = Some(error);
            start
        }
    }
}
