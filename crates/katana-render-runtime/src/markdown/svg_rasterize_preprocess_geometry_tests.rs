use super::materialize_geometry_properties;

fn preprocess(source: &str) -> String {
    materialize_geometry_properties(source.to_owned())
}

#[test]
fn stylesheet_important_overrides_geometry_attributes() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.round { rx: 12px !important; ry: 11px !important; }</style><rect class="round" rx="5" ry="4"/></svg>"#;
    let actual = preprocess(source);
    assert!(actual.contains("rx=\"12px\""));
    assert!(actual.contains("ry=\"11px\""));
}

#[test]
fn important_stylesheet_beats_normal_inline_and_inline_important_wins_tie() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.shape { rx: 8px !important; ry: 9px !important; }</style><rect class="shape" rx="5" ry="5" style="rx: 10px; ry: 10px !important"/></svg>"#;
    let actual = preprocess(source);
    assert!(actual.contains("rx=\"8px\""));
    assert!(actual.contains("ry=\"10px\""));
}

#[test]
fn later_same_specificity_rule_wins_and_stylesheet_beats_attribute() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.shape { rx: 7px; }.shape { rx: 9px; }</style><rect class="shape" rx="5"/></svg>"#;
    let actual = preprocess(source);
    assert!(actual.contains("rx=\"9px\""));
}

#[test]
fn higher_specificity_wins_even_when_a_lower_specificity_rule_is_later() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>#target { rx: 10px; }.shape { rx: 9px; }</style><rect id="target" class="shape" rx="5"/></svg>"#;
    assert!(preprocess(source).contains("rx=\"10px\""));
}

#[test]
fn malformed_and_negative_declarations_keep_existing_attribute() {
    let malformed = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.shape { rx: not-a-length; }</style><rect class="shape" rx="5"/></svg>"#;
    let negative = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.shape { rx: -2px; }</style><rect class="shape" rx="5"/></svg>"#;
    assert_eq!(preprocess(malformed), malformed);
    assert_eq!(preprocess(negative), negative);
}

#[test]
fn malformed_later_declaration_does_not_hide_valid_geometry_value() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.shape { rx: 12px; rx: bogus; }</style><rect class="shape" rx="5"/></svg>"#;
    assert!(preprocess(source).contains("rx=\"12px\""));
}

#[test]
fn unsupported_complex_values_preserve_existing_attribute() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.shape { rx: calc(10px + 2px); }</style><rect class="shape" rx="5"/></svg>"#;
    assert_eq!(preprocess(source), source);
}

#[test]
fn negative_zero_is_materialized_as_nonnegative_zero() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.shape { rx: -0px; }</style><rect class="shape" rx="5"/></svg>"#;
    assert!(preprocess(source).contains("rx=\"0px\""));
}

#[test]
fn geometry_attribute_selector_preserves_original_svg_bytes() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>rect[rx="5"] { fill: red; } rect { rx: 12px; }</style><rect rx="5"/></svg>"#;
    assert_eq!(preprocess(source), source);
}

#[test]
fn no_namespace_svg_markup_keeps_supported_attribute_selectors_unchanged() {
    let source =
        r#"<svg><style>rect[rx="5"] { fill: red; } rect { rx: 12px; }</style><rect rx="5"/></svg>"#;
    assert_eq!(preprocess(source), source);
}

#[test]
fn foreign_namespace_style_matches_usvg_and_guards_attribute_selector_changes() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:x="urn:other"><x:style>rect[rx="5"] { fill: red; }</x:style><rect rx="5" style="rx: 12px !important"/></svg>"#;
    assert_eq!(preprocess(source), source);
}

#[test]
fn missing_geometry_attribute_is_added_for_rect_and_ellipse() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.round { rx: 6px; }</style><rect class="round"/><ellipse class="round"/> </svg>"#;
    let actual = preprocess(source);
    assert_eq!(actual.matches("rx=\"6px\"").count(), 2);
    assert!(actual.contains("<rect class=\"round\" rx=\"6px\"/>"));
    assert!(actual.contains("<ellipse class=\"round\" rx=\"6px\"/>"));
}

#[test]
fn auto_removes_an_existing_geometry_attribute() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.round { rx: auto; }</style><rect class="round" rx="6px"/></svg>"#;
    let actual = preprocess(source);
    assert!(actual.contains("<rect class=\"round\"/>"));
}

#[test]
fn unrelated_svg_and_attribute_only_markup_keep_original_bytes() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.shape { fill: red; }</style><rect rx="5"/></svg>"#;
    let without_geometry_css = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect rx="5"/></svg>"#;
    assert_eq!(preprocess(source), source);
    assert_eq!(preprocess(without_geometry_css), without_geometry_css);
}
