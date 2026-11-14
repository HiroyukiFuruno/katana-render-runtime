use super::super::style::{CssBorderStyle, CssBoxSizing};
use super::{CssStyle, input_geometry, is_checkbox};

#[test]
fn auto_height_input_includes_line_height_padding_and_border() {
    let mut style = CssStyle::browser_default();
    style.line_height = 33.6;
    style.padding_top = 16.0;
    style.padding_bottom = 16.0;
    style.border_width = 1.0;
    style.border_style = CssBorderStyle::Solid;

    let geometry = input_geometry(0.0, 0.0, 300.0, &style);

    assert!((geometry.height - 67.6).abs() < 0.01);
}

#[test]
fn border_box_explicit_input_height_is_not_expanded() {
    let mut style = CssStyle::browser_default();
    style.box_sizing = CssBoxSizing::BorderBox;
    style.height = Some(40.0);
    style.padding_top = 8.0;
    style.padding_bottom = 8.0;

    assert_eq!(input_geometry(0.0, 0.0, 300.0, &style).height, 40.0);
}

#[test]
fn checkbox_type_attribute_is_detected() {
    let attrs = vec![("type".to_string(), "checkbox".to_string())];

    assert!(is_checkbox(&attrs));
}
