use super::super::super::html_document::HtmlDocumentNode;
use super::super::constants::MIN_LAYOUT_WIDTH;
use super::super::layout_media::resolved_svg_width;
use super::super::style::CssStyle;
use super::intrinsic::intrinsic_layout_width;
use taffy::prelude::Display;

pub(super) fn stretched_column_widths(styles: &[CssStyle], width: f32) -> Vec<f32> {
    styles
        .iter()
        .map(|style| {
            let explicit_width = style.explicit_width(width);
            let measured = explicit_width.unwrap_or(width);
            let measured = if explicit_width.is_some() {
                measured
            } else {
                measured.min(width)
            };
            enforce_min_width(style, measured, width).max(MIN_LAYOUT_WIDTH)
        })
        .collect()
}

pub(super) fn non_grid_measured_widths(
    nodes: &[&HtmlDocumentNode],
    styles: &[CssStyle],
    width: f32,
    parent: &CssStyle,
) -> Vec<f32> {
    nodes
        .iter()
        .zip(styles)
        .map(|(node, style)| {
            let explicit_width = style.explicit_width(width);
            let measured = explicit_width
                .or_else(|| resolved_svg_width(node, width, style))
                .unwrap_or_else(|| intrinsic_layout_width(node, style, width));
            let measured = if explicit_width.is_some() && is_nonstretch_flex_column(parent) {
                measured
            } else {
                flex_column_cross_size(measured, width, parent)
            };
            enforce_min_width(style, measured, width)
        })
        .map(|measured| measured.max(MIN_LAYOUT_WIDTH))
        .collect()
}

fn flex_column_cross_size(measured: f32, available: f32, parent: &CssStyle) -> f32 {
    if is_nonstretch_flex_column(parent) {
        measured.min(available)
    } else {
        measured
    }
}

fn is_nonstretch_flex_column(parent: &CssStyle) -> bool {
    parent.display == Display::Flex
        && matches!(
            parent.flex_direction,
            taffy::style::FlexDirection::Column | taffy::style::FlexDirection::ColumnReverse
        )
        && !matches!(
            parent.align_items,
            None | Some(taffy::style::AlignItems::STRETCH | taffy::style::AlignItems::NORMAL)
        )
}

pub(super) fn enforce_min_width(style: &CssStyle, measured: f32, available: f32) -> f32 {
    style
        .minimum_outer_width(available)
        .map_or(measured, |minimum| measured.max(minimum))
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, is_nonstretch_flex_column};
    use taffy::prelude::Display;
    use taffy::style::{AlignItems, FlexDirection};

    #[test]
    fn column_alignment_classifies_stretch_omission_and_center() {
        let flex_column = CssStyle {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            ..CssStyle::browser_default()
        };
        let stretch = CssStyle {
            align_items: Some(AlignItems::STRETCH),
            ..flex_column.clone()
        };
        let normal = CssStyle {
            align_items: Some(AlignItems::NORMAL),
            ..flex_column.clone()
        };
        let omitted = CssStyle {
            align_items: None,
            ..flex_column.clone()
        };
        let centered = CssStyle {
            align_items: Some(AlignItems::CENTER),
            ..flex_column
        };

        assert!(!is_nonstretch_flex_column(&stretch));
        assert!(!is_nonstretch_flex_column(&normal));
        assert!(!is_nonstretch_flex_column(&omitted));
        assert!(is_nonstretch_flex_column(&centered));
    }
}
