use super::{CssStyle, CssVerticalAlign, normalized_property_name};
use std::borrow::Cow;

#[test]
fn property_name_helper_matches_ascii_case_oracle_and_borrowing() {
    for input in [
        "", "color", "COLOR", " Color ", "éColor", "É", "Ω", "ÉColor", "--x",
    ] {
        let expected = input.to_ascii_lowercase();
        let normalized = normalized_property_name(input);
        assert_eq!(normalized.as_ref(), expected);
        if input.bytes().any(|byte| byte.is_ascii_uppercase()) {
            assert!(matches!(normalized, Cow::Owned(_)));
        } else {
            assert!(matches!(normalized, Cow::Borrowed(_)));
        }
    }
}

#[test]
fn mixed_case_names_match_lowercase_across_dispatch_paths() {
    let mut apply_lower = CssStyle::browser_default();
    let mut apply_mixed = CssStyle::browser_default();
    apply_lower.apply("border-top-width", "3px");
    apply_mixed.apply("BoRdEr-ToP-WiDtH", "3px");
    assert_eq!(apply_lower.border_top_width, apply_mixed.border_top_width);

    let mut border_lower = CssStyle::browser_default();
    let mut border_mixed = CssStyle::browser_default();
    border_lower.apply_border_declaration("border-top-color", "#123456");
    border_mixed.apply_border_declaration("BoRdEr-ToP-CoLoR", "#123456");
    assert_eq!(border_lower.border_top_color, border_mixed.border_top_color);

    let mut box_lower = CssStyle::browser_default();
    let mut box_mixed = CssStyle::browser_default();
    box_lower.apply_box_property("margin-left", "7px");
    box_mixed.apply_box_property("MaRgIn-LeFt", "7px");
    assert_eq!(box_lower.margin_left, box_mixed.margin_left);

    let mut typography_lower = CssStyle::browser_default();
    let mut typography_mixed = CssStyle::browser_default();
    typography_lower.apply_typography_property("font-family", "Inter UI");
    typography_mixed.apply_typography_property("FoNt-FaMiLy", "Inter UI");
    assert_eq!(typography_lower.font_family, "Inter UI");
    assert_eq!(typography_mixed.font_family, typography_lower.font_family);
}

#[test]
fn mixed_case_border_style_and_color_match_lowercase() {
    let mut lower = CssStyle::browser_default();
    let mut mixed = CssStyle::browser_default();
    lower.apply_border_declaration("border-top-style", "dashed");
    lower.apply_border_declaration("border-top-color", "#123456");
    mixed.apply_border_declaration("BoRdEr-ToP-StYlE", "dashed");
    mixed.apply_border_declaration("BoRdEr-ToP-CoLoR", "#123456");
    assert_eq!(mixed.border_top_style, lower.border_top_style);
    assert_eq!(mixed.border_top_color, lower.border_top_color);
}

#[test]
fn mixed_case_box_dimensions_margins_and_overflow_match_lowercase() {
    let mut lower = CssStyle::browser_default();
    let mut mixed = CssStyle::browser_default();
    lower.apply_box_property("width", "12px");
    lower.apply_box_property("margin-left", "7px");
    lower.apply_box_property("overflow", "hidden");
    mixed.apply_box_property("WiDtH", "12px");
    mixed.apply_box_property("MaRgIn-LeFt", "7px");
    mixed.apply_box_property("OvErFlOw", "hidden");
    assert_eq!(mixed.width, lower.width);
    assert_eq!(mixed.margin_left, lower.margin_left);
    assert_eq!(mixed.overflow, lower.overflow);
}

#[test]
fn mixed_case_font_properties_preserve_value_case() {
    let mut lower = CssStyle::browser_default();
    let mut mixed = CssStyle::browser_default();
    lower.apply_typography_property("font-size", "18px");
    lower.apply_typography_property("font-weight", "600");
    lower.apply_typography_property("font-family", "Inter UI");
    mixed.apply_typography_property("FoNt-SiZe", "18px");
    mixed.apply_typography_property("FoNt-WeIgHt", "600");
    mixed.apply_typography_property("FoNt-FaMiLy", "Inter UI");
    assert_eq!(mixed.font_size, lower.font_size);
    assert_eq!(mixed.font_weight, lower.font_weight);
    assert_eq!(mixed.font_family, "Inter UI");
}

#[test]
fn spaces_and_non_ascii_property_names_remain_unrecognized() {
    let mut style = CssStyle::browser_default();
    style.vertical_align = CssVerticalAlign::Bottom;
    let before = style.clone();
    style.apply(" border-top-width ", "9px");
    style.apply("éolor", "#ffffff");
    assert_eq!(style.border_top_width, before.border_top_width);
    assert_eq!(style.color, before.color);
    assert_eq!(style.vertical_align, CssVerticalAlign::Bottom);
}

#[test]
fn parsed_multicolumn_shorthand_and_gap_reach_layout_style() -> Result<(), String> {
    use crate::renderer::backends::html_document::HtmlDocument;
    let html = "<div id=columns style='columns: 3 80px; column-gap: 12px'>text</div>";
    let document = HtmlDocument::parse(html);
    let nodes = document.interactive_nodes_with_styles(&std::collections::HashMap::new());
    let (tag, attributes) = find_columns_element(&nodes).ok_or("columns element missing")?;
    let style = CssStyle::from_element(tag, attributes, &CssStyle::browser_default());
    assert_eq!(style.column_count, Some(3));
    assert_eq!(style.column_width, Some(80.0));
    assert_eq!(style.column_gap, 12.0);
    assert_eq!(style.flow_column_gap, Some(12.0));
    Ok(())
}

fn find_columns_element(
    nodes: &[crate::renderer::backends::html_document::HtmlDocumentNode],
) -> Option<(&str, &[(String, String)])> {
    use crate::renderer::backends::html_document::HtmlDocumentNode;
    nodes.iter().find_map(|node| match node {
        HtmlDocumentNode::Element {
            tag,
            attributes,
            children,
            ..
        } => {
            if attributes
                .iter()
                .any(|(name, value)| name == "id" && value == "columns")
            {
                Some((tag.as_str(), attributes.as_slice()))
            } else {
                find_columns_element(children)
            }
        }
        HtmlDocumentNode::Text(_) => None,
    })
}
