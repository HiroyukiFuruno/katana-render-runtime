use super::super::html_document::HtmlDocumentNode;
use super::constants::{BUTTON_UA_BORDER_WIDTH, BUTTON_UA_HORIZONTAL_PADDING};
use super::document::{attribute, node_text};
use super::style::{CssBorderStyle, CssStyle};
use super::text_metrics::text_width;

const UA_CONTROL_FONT_SIZE: f32 = 13.3333;
const UA_CONTROL_LINE_HEIGHT: f32 = 15.3333;

pub(super) fn ua_control_style(style: &CssStyle) -> CssStyle {
    if style.explicit_typography {
        return style.clone();
    }
    let mut style = style.clone();
    style.font_family = "Arial".to_string();
    style.font_size = UA_CONTROL_FONT_SIZE;
    style.line_height = UA_CONTROL_LINE_HEIGHT;
    style.line_height_factor = None;
    style.normal_line_height = false;
    style
}

pub(super) fn button_width(children: &[HtmlDocumentNode], width: f32, style: &CssStyle) -> f32 {
    if style.explicit_width {
        return style.explicit_width(width).unwrap_or(width);
    }
    (text_width(&node_text(children), style)
        + style.padding_left
        + style.padding_right
        + style.border_left_width()
        + style.border_right_width())
    .min(width)
}

pub(super) fn button_style(style: &CssStyle) -> CssStyle {
    let mut style = ua_control_style(style);
    if !style.explicit_background {
        style.background = Some("#efefef".to_string());
    }
    let has_author_border = style.explicit_border;
    style.border = Some(
        style
            .border
            .clone()
            .unwrap_or_else(|| "#6c757d".to_string()),
    );
    if !has_author_border {
        style.border_style = CssBorderStyle::Solid;
        style.border_width = BUTTON_UA_BORDER_WIDTH;
    }
    if !style.explicit_padding {
        style.padding_left = BUTTON_UA_HORIZONTAL_PADDING;
        style.padding_right = BUTTON_UA_HORIZONTAL_PADDING;
    }
    style
}

pub(super) fn input_style(style: &CssStyle, focused: bool) -> CssStyle {
    let mut style = style.clone();
    style.background = Some(
        style
            .background
            .clone()
            .unwrap_or_else(|| "#ffffff".to_string()),
    );
    if focused {
        style.border = Some("#0969da".to_string());
    } else if style.border.is_none() {
        style.border = Some("#767676".to_string());
    }
    if !style.explicit_border {
        style.border_style = CssBorderStyle::Solid;
    }
    if style.border_width == 0.0 {
        style.border_width = 2.0;
    }
    style
}

pub(super) fn visible_details_children(
    attributes: &[(String, String)],
    children: &[HtmlDocumentNode],
) -> Vec<HtmlDocumentNode> {
    let open = details_is_open(attributes);
    children
        .iter()
        .filter(|child| is_summary(child) || open)
        .cloned()
        .collect()
}

pub(super) fn details_is_open(attributes: &[(String, String)]) -> bool {
    attribute(attributes, "open").is_some()
}

pub(super) fn summary_height(style: &CssStyle) -> f32 {
    (style.height.unwrap_or(style.line_height) + style.padding_top + style.padding_bottom)
        .max(style.min_height)
}

fn is_summary(node: &&HtmlDocumentNode) -> bool {
    matches!(node, HtmlDocumentNode::Element { tag, .. } if tag == "summary")
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, button_style, button_width, ua_control_style};
    use crate::renderer::backends::html_document::HtmlDocumentNode;

    #[test]
    fn button_style_keeps_an_explicit_background() {
        let style = CssStyle {
            background: Some("#123456".to_string()),
            explicit_background: true,
            ..CssStyle::browser_default()
        };

        assert_eq!(button_style(&style).background.as_deref(), Some("#123456"));
    }

    #[test]
    fn ua_control_typography_is_used_only_without_author_typography() {
        let inherited = CssStyle::browser_default();
        let ua_input = CssStyle::from_element("input", &[], &inherited);
        let ua_input = ua_control_style(&ua_input);
        assert_eq!(ua_input.font_family, "Arial");
        assert!((ua_input.font_size - 13.3333).abs() < 0.001);

        let authored = CssStyle::from_element(
            "input",
            &[("style".to_string(), "font: 20px Inter".to_string())],
            &inherited,
        );
        let authored = ua_control_style(&authored);
        assert_eq!(authored.font_family, "Inter");
        assert_eq!(authored.font_size, 20.0);
    }

    #[test]
    fn button_ua_box_metrics_include_padding_and_border() {
        let style = button_style(&CssStyle::browser_default());
        assert_eq!(style.padding_left, 6.0);
        assert_eq!(style.padding_right, 6.0);
        assert_eq!(style.border_width, 2.0);
        assert_eq!(style.border_left_width(), 2.0);
        assert_eq!(style.border_right_width(), 2.0);

        let children = [HtmlDocumentNode::Text("保存".to_string())];
        let width = button_width(&children, 1_000.0, &style);
        assert!(
            (width - 42.6666).abs() < 0.01,
            "unexpected UA button width: {width}"
        );
    }

    #[test]
    fn button_author_zero_padding_is_preserved() {
        let inherited = CssStyle::browser_default();
        let authored = CssStyle::from_element(
            "button",
            &[("style".to_string(), "padding:0;border:0".to_string())],
            &inherited,
        );
        let style = button_style(&authored);
        assert_eq!(style.padding_left, 0.0);
        assert_eq!(style.padding_right, 0.0);
        assert_eq!(style.border_width, 0.0);
    }
}
