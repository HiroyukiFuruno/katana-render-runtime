use super::super::html_document::HtmlDocumentNode;
use super::constants::BUTTON_TEXT_LEFT_PADDING;
use super::document::node_text;
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::text_metrics::text_width;
use super::types::{ControlLayout, ElementRenderContext, HitTarget, HitTargetKind};
use taffy::style::{AlignContentKeyword, AlignItemsKeyword, Display, FlexDirection};

#[derive(Clone, Copy)]
struct ControlTextLayout {
    x: f32,
    width: f32,
    y: f32,
    height: f32,
}

impl HtmlLayoutRenderer {
    pub(super) fn paint_button(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: ControlLayout<'_>,
    ) {
        self.paint_box(
            layout.x,
            layout.y,
            layout.width,
            layout.height,
            layout.style,
        );
        self.paint_button_text(element.children, layout);
        self.push_target(
            element.node_id,
            layout.x,
            layout.y,
            layout.width,
            layout.height,
            HitTargetKind::Click,
        );
    }

    fn paint_button_text(&mut self, children: &[HtmlDocumentNode], layout: ControlLayout<'_>) {
        let text = node_text(children);
        let text_layout = button_text_layout(&text, layout);
        self.paint_control_text(
            &text,
            text_layout.x,
            text_layout.width,
            text_layout.y,
            text_layout.height,
            layout.style,
        );
    }

    pub(super) fn paint_control_text(
        &mut self,
        text: &str,
        x: f32,
        width: f32,
        start: f32,
        height: f32,
        style: &CssStyle,
    ) {
        let baseline = start + (height - style.line_height) / 2.0 + style.font_size;
        self.paint_text_lines(&[text.to_string()], x, width, baseline, style);
    }

    pub(super) fn push_target(
        &mut self,
        node_id: u64,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        kind: HitTargetKind,
    ) {
        self.hit_targets.push(HitTarget {
            node_id,
            x,
            y,
            width,
            height,
            kind,
        });
    }
}

fn button_text_layout(text: &str, layout: ControlLayout<'_>) -> ControlTextLayout {
    let mut text_layout = ControlTextLayout {
        x: layout.x + BUTTON_TEXT_LEFT_PADDING,
        width: (layout.width - BUTTON_TEXT_LEFT_PADDING * 2.0).max(0.0),
        y: layout.y,
        height: layout.height,
    };
    let Some((horizontal_centered, vertical_centered)) = flex_center_axes(layout.style) else {
        return text_layout;
    };

    if horizontal_centered {
        let content_x = layout.x + layout.style.border_left_width() + layout.style.padding_left;
        let content_width = layout.style.content_width(layout.width);
        let measured_width = text_width(text, layout.style).min(content_width);
        text_layout.x = content_x + (content_width - measured_width) / 2.0;
        text_layout.width = measured_width;
    }
    if vertical_centered {
        text_layout.y = layout.y + layout.style.border_top_width() + layout.style.padding_top;
        text_layout.height = layout.style.content_height(layout.height);
    }
    text_layout
}

fn flex_center_axes(style: &CssStyle) -> Option<(bool, bool)> {
    (style.display == Display::Flex).then(|| {
        let row_axis = matches!(
            style.flex_direction,
            FlexDirection::Row | FlexDirection::RowReverse
        );
        /* WHY: 従来の安全指定による配置を変えないため、既定と unsafe の中心位置だけを同じ軸へ投影する。 */
        let main_centered = style.justify_content.is_some_and(|alignment| {
            !alignment.is_safe() && alignment.keyword() == AlignContentKeyword::Center
        });
        let cross_centered = style.align_items.is_some_and(|alignment| {
            !alignment.is_safe() && alignment.keyword() == AlignItemsKeyword::Center
        });
        if row_axis {
            (main_centered, cross_centered)
        } else {
            (cross_centered, main_centered)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{ControlLayout, button_text_layout, flex_center_axes};
    use crate::renderer::backends::html_interactive::constants::BUTTON_TEXT_LEFT_PADDING;
    use crate::renderer::backends::html_interactive::{
        style::{CssBorderStyle, CssStyle},
        text_metrics::text_width,
    };
    use taffy::style::{AlignItems, Display, FlexDirection, JustifyContent};

    #[test]
    fn parsed_center_axes_preserve_safety_and_direction() {
        let alignments = [
            (None, false),
            (Some("center"), true),
            (Some("unsafe center"), true),
            (Some("safe center"), false),
            (Some("end"), false),
            (Some("unsafe end"), false),
            (Some("safe end"), false),
            (Some("flex-end"), false),
            (Some("unsafe flex-end"), false),
            (Some("safe flex-end"), false),
        ];
        for (direction, row_axis) in [
            ("row", true),
            ("row-reverse", true),
            ("column", false),
            ("column-reverse", false),
        ] {
            for property in ["align-items", "justify-content"] {
                for (alignment, centered) in alignments {
                    assert_parsed_center_case(direction, row_axis, property, alignment, centered);
                }
            }
        }
    }

    fn assert_parsed_center_case(
        direction: &str,
        row_axis: bool,
        property: &str,
        alignment: Option<&str>,
        centered: bool,
    ) {
        let mut style = parsed_center_style(direction, property, alignment);
        let horizontal = row_axis == (property == "justify-content");
        let expected = if horizontal {
            (centered, false)
        } else {
            (false, centered)
        };
        assert_eq!(
            flex_center_axes(&style),
            Some(expected),
            "{direction} {property}: {alignment:?}"
        );
        style.apply("display", "block");
        assert_eq!(flex_center_axes(&style), None);
    }

    fn parsed_center_style(direction: &str, property: &str, alignment: Option<&str>) -> CssStyle {
        let mut style = CssStyle::browser_default();
        style.apply("display", "flex");
        style.apply("flex-direction", direction);
        assert_eq!(style.align_items, None);
        assert_eq!(style.justify_content, None);
        if let Some(alignment) = alignment {
            style.apply(property, alignment);
            assert!(
                style.align_items.is_some() || style.justify_content.is_some(),
                "{property}: {alignment}"
            );
        }
        style
    }

    #[test]
    fn column_flex_centers_button_text_on_cross_and_main_axes() {
        let mut style = CssStyle::browser_default();
        style.display = Display::Flex;
        style.flex_direction = FlexDirection::Column;
        style.align_items = Some(AlignItems::CENTER);
        style.justify_content = Some(JustifyContent::CENTER);
        style.border_width = 1.0;
        style.border_style = CssBorderStyle::Solid;
        let layout = ControlLayout {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 40.0,
            style: &style,
        };

        let text_layout = button_text_layout("‹", layout);
        let measured_width = text_width("‹", &style);

        assert!((text_layout.x - (20.0 - measured_width / 2.0)).abs() <= f32::EPSILON);
        assert!((text_layout.width - measured_width).abs() <= f32::EPSILON);
        assert!((text_layout.y - 1.0).abs() <= f32::EPSILON);
        assert!((text_layout.height - 38.0).abs() <= f32::EPSILON);
    }

    #[test]
    fn row_flex_only_horizontally_centers_button_text() {
        let mut style = CssStyle::browser_default();
        style.display = Display::Flex;
        style.flex_direction = FlexDirection::Row;
        style.justify_content = Some(JustifyContent::CENTER);
        let layout = ControlLayout {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 20.0,
            style: &style,
        };
        let text = "abc";

        let text_layout = button_text_layout(text, layout);
        let measured_width = text_width(text, &style);
        let content_width = style.content_width(layout.width);
        let expected_x = style.border_left_width()
            + style.padding_left
            + (content_width - measured_width.min(content_width)) / 2.0;

        assert!((text_layout.x - expected_x).abs() <= f32::EPSILON);
        assert_eq!(text_layout.width, measured_width.min(content_width));
        assert!((text_layout.y - 0.0).abs() <= f32::EPSILON);
        assert_eq!(text_layout.height, 20.0);
    }

    #[test]
    fn column_flex_only_vertically_centers_button_text() {
        let mut style = CssStyle::browser_default();
        style.display = Display::Flex;
        style.flex_direction = FlexDirection::Column;
        style.justify_content = Some(JustifyContent::CENTER);
        style.border_width = 0.0;
        style.padding_top = 0.0;
        let layout = ControlLayout {
            x: 0.0,
            y: 5.0,
            width: 40.0,
            height: 40.0,
            style: &style,
        };

        let text_layout = button_text_layout("abc", layout);
        let expected_y = layout.y + style.border_top_width() + style.padding_top;
        let expected_x = layout.x + BUTTON_TEXT_LEFT_PADDING;

        assert_eq!(text_layout.x, expected_x);
        assert_eq!(text_layout.y, expected_y);
        assert_eq!(text_layout.height, style.content_height(layout.height));
        assert_eq!(
            text_layout.width,
            layout.width - BUTTON_TEXT_LEFT_PADDING * 2.0
        );
    }
}
