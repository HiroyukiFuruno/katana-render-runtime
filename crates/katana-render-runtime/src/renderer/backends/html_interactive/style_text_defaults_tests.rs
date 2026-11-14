use super::{CssBoxSizing, CssGridTrack, CssStyle};

const VIEWPORT_WIDTH: f32 = 640.0;
const VIEWPORT_HEIGHT: f32 = 480.0;
const DEFAULT_GRID_FRACTION: f32 = 1.0;
const PARENT_COLOR: &str = "#123456";
const CHILD_COLOR: &str = "#abcdef";
const PARENT_FONT: &str = "Inter UI, sans-serif";
const CHILD_FONT: &str = "Noto Sans, sans-serif";
const CODE_FONT: &str = "monospace";

#[test]
fn text_defaults_inherit_across_div_paragraph_and_span_and_allow_override() {
    let parent = CssStyle::from_element(
        "div",
        &[(
            "style".to_string(),
            format!("color:{PARENT_COLOR};font-family:{PARENT_FONT}"),
        )],
        &CssStyle::browser_default(),
    );
    let paragraph = CssStyle::from_element("p", &[], &parent);
    let inherited = CssStyle::from_element("span", &[], &paragraph);
    let authored = CssStyle::from_element(
        "span",
        &[(
            "style".to_string(),
            format!("color:{CHILD_COLOR};font-family:{CHILD_FONT}"),
        )],
        &paragraph,
    );

    assert_eq!(inherited.color, PARENT_COLOR);
    assert_eq!(inherited.font_family, PARENT_FONT);
    assert_eq!(authored.color, CHILD_COLOR);
    assert_eq!(authored.font_family, CHILD_FONT);
}

#[test]
fn browser_defaults_preserve_viewport_grid_and_line_height() {
    let parent = CssStyle::browser_default_for_viewport(VIEWPORT_WIDTH, VIEWPORT_HEIGHT);
    let child = CssStyle::from_element("div", &[], &parent);

    assert_eq!(child.viewport_width, VIEWPORT_WIDTH);
    assert_eq!(child.viewport_height, VIEWPORT_HEIGHT);
    assert_eq!(
        child.grid_template_columns,
        vec![CssGridTrack::Fraction(DEFAULT_GRID_FRACTION)]
    );
    assert_eq!(child.line_height, parent.line_height);
    assert_eq!(child.line_height_factor, parent.line_height_factor);
}

#[test]
fn tag_defaults_keep_code_font_and_form_control_box_sizing() {
    let inherited = CssStyle::browser_default();
    let code = CssStyle::from_element("code", &[], &inherited);
    let form = CssStyle::from_element("form", &[], &inherited);
    let input = CssStyle::from_element("input", &[], &form);
    let textarea = CssStyle::from_element("textarea", &[], &form);

    assert_eq!(code.font_family, CODE_FONT);
    assert_eq!(form.box_sizing, CssBoxSizing::ContentBox);
    assert_eq!(input.box_sizing, CssBoxSizing::BorderBox);
    assert_eq!(textarea.box_sizing, CssBoxSizing::BorderBox);
}
