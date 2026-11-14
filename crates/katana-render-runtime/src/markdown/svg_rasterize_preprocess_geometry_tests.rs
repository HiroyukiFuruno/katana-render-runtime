use super::materialize_geometry_properties;

#[path = "svg_rasterize_preprocess_geometry_parser_tests.rs"]
mod parser_tests;

type Rgb = (u8, u8, u8);
type InlinePriorityCase = (bool, bool, Rgb);
type ParsedPaints = (Option<Rgb>, Option<Rgb>);
type StylesheetCascadeCase = (&'static str, Rgb);

const FIRST_STROKE: Rgb = (0x11, 0x22, 0x33);
const LAST_STROKE: Rgb = (0x33, 0x44, 0x55);
const INLINE_CASCADE_STROKE: Rgb = (0x99, 0xaa, 0xbb);
const UNRELATED_FILL: Rgb = (0x66, 0x77, 0x88);
const INLINE_PRIORITY_CASES: [InlinePriorityCase; 4] = [
    (false, false, LAST_STROKE),
    (false, true, LAST_STROKE),
    (true, false, FIRST_STROKE),
    (true, true, LAST_STROKE),
];
const STYLESHEET_CASCADE_CASES: [StylesheetCascadeCase; 4] = [
    (
        "<style>.shape{stroke:#334455}</style><path class='shape' d='M1 1 H11 V11 Z' stroke='#112233'/>",
        LAST_STROKE,
    ),
    (
        "<style>.shape{stroke:#112233}.shape{stroke:#334455}</style><path class='shape' d='M1 1 H11 V11 Z' stroke='#667788' style='stroke:#99aabb'/>",
        INLINE_CASCADE_STROKE,
    ),
    (
        "<style>.shape{stroke:#112233!important}.shape{stroke:#334455!important}</style><path class='shape' d='M1 1 H11 V11 Z' style='stroke:#667788'/>",
        LAST_STROKE,
    ),
    (
        "<style>.shape{stroke:#112233!important}.shape{stroke:#334455!important}</style><path class='shape' d='M1 1 H11 V11 Z' style='stroke:#99aabb!important'/>",
        INLINE_CASCADE_STROKE,
    ),
];

fn preprocess(source: &str) -> String {
    materialize_geometry_properties(source.to_owned())
}

#[test]
fn usvg_inline_duplicate_declarations_respect_priority_then_source_order() -> Result<(), String> {
    for (first_important, second_important, expected_stroke) in INLINE_PRIORITY_CASES {
        let (fill, stroke) = parse_inline_paints(first_important, second_important)?;
        assert_eq!(
            fill,
            Some(UNRELATED_FILL),
            "unrelated fill property must remain intact"
        );
        assert_eq!(
            stroke,
            Some(expected_stroke),
            "first important={first_important}, second important={second_important}"
        );
    }
    Ok(())
}

#[test]
fn usvg_stylesheet_inline_and_presentation_cascade_order_is_preserved() -> Result<(), String> {
    for (content, expected_stroke) in STYLESHEET_CASCADE_CASES {
        let svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='12' height='12'>{content}</svg>"
        );
        assert_eq!(
            parse_usvg_paints(&svg)?.1,
            Some(expected_stroke),
            "{content}"
        );
    }
    Ok(())
}

fn parse_inline_paints(
    first_important: bool,
    second_important: bool,
) -> Result<ParsedPaints, String> {
    let first_priority = if first_important { " !important" } else { "" };
    let second_priority = if second_important { " !important" } else { "" };
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12"><path d="M1 1 H11 V11 Z" style="fill:#667788;stroke:#112233{first_priority};stroke:#334455{second_priority}"/></svg>"##
    );
    parse_usvg_paints(&svg)
}

fn parse_usvg_paints(svg: &str) -> Result<ParsedPaints, String> {
    use crate::svg_engine::usvg::{Node, Options, Tree};

    let tree = Tree::from_str(svg, &Options::default()).map_err(|error| error.to_string())?;
    let path = match tree.root().children().first() {
        Some(Node::Path(path)) => path,
        _ => return Err("inline cascade fixture path missing".to_owned()),
    };
    Ok((
        path.fill().and_then(|fill| paint_rgb(fill.paint())),
        path.stroke().and_then(|stroke| paint_rgb(stroke.paint())),
    ))
}

fn paint_rgb(paint: &crate::svg_engine::usvg::Paint) -> Option<Rgb> {
    match paint {
        crate::svg_engine::usvg::Paint::Color(color) => Some((color.red, color.green, color.blue)),
        _ => None,
    }
}

#[test]
fn stylesheet_important_overrides_geometry_attributes() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.round { rx: 12px !important; ry: 11px !important; }</style><rect class="round" rx="5" ry="4"/></svg>"#;
    let actual = preprocess(source);
    assert!(actual.contains("rx=\"12px\""));
    assert!(actual.contains("ry=\"11px\""));
}

#[test]
fn geometry_preprocessing_preserves_malformed_svg_bytes() {
    let source =
        "<svg xmlns='http://www.w3.org/2000/svg'><style>rect{rx:12px}</style><rect rx='5'></svg>";
    assert_eq!(preprocess(source), source);
}

#[test]
fn geometry_preprocessing_ignores_non_css_style_blocks() {
    let source = "<svg xmlns='http://www.w3.org/2000/svg'><style type='text/css'>rect{rx:8px}</style><style type='text/plain'>rect{rx:12px}</style><rect rx='5'/></svg>";
    let actual = preprocess(source);
    assert!(actual.contains("<rect rx='8px'"), "{actual}");
    assert!(actual.contains("<style type='text/plain'>rect{rx:12px}</style>"));
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
fn css_wide_geometry_keywords_override_but_trailing_tokens_do_not() {
    for keyword in [
        "inherit",
        "initial",
        "unset",
        "revert",
        "revert-layer",
        "INHERIT",
    ] {
        let source = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><style>.shape {{ rx: 12px; rx: {keyword}; }}</style><rect class=\"shape\" rx=\"5\"/></svg>"
        );
        assert_eq!(preprocess(&source), source, "keyword={keyword}");

        let invalid = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><style>.shape {{ rx: 12px; rx: {keyword} extra; }}</style><rect class=\"shape\" rx=\"5\"/></svg>"
        );
        assert!(
            preprocess(&invalid).contains("rx=\"12px\""),
            "keyword={keyword}"
        );
    }
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
    let auto = source.replace("12px", "auto");
    assert_eq!(preprocess(&auto), auto);
}

#[test]
fn ancestor_sibling_and_first_child_selectors_survive_geometry_materialization() {
    let cases = [
        (
            "ancestor",
            r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.wrapper > rect.target { rx: 12px; }</style><g class="wrapper"><rect class="target" rx="5"/></g></svg>"#,
        ),
        (
            "previous sibling",
            r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.before + rect.target { rx: 12px; }</style><g><rect class="before"/><rect class="target" rx="5"/></g></svg>"#,
        ),
        (
            "first child",
            r#"<svg xmlns="http://www.w3.org/2000/svg"><style>rect.target:first-child { rx: 12px; }</style><g><rect class="target" rx="5"/><rect/></g></svg>"#,
        ),
    ];
    for (selector_kind, source) in cases {
        assert!(
            preprocess(source).contains("rx=\"12px\""),
            "{selector_kind} selector changed during geometry materialization"
        );
    }
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
fn geometry_attribute_is_inserted_after_quoted_values_in_both_tag_forms() {
    let attributes = "data-label='形>状' data-detail=\"'>'\"";
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><style>rect{{rx:6px}}</style><title>形状</title><rect {attributes}><title>shape</title></rect><rect {attributes}/></svg>"#
    );
    let expected = source.replace(attributes, &format!("{attributes} rx=\"6px\""));
    assert_eq!(preprocess(&source), expected);
}

#[test]
fn auto_removes_an_existing_geometry_attribute() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.round { rx: auto; }</style><rect class="round" rx="6px"/></svg>"#;
    let actual = preprocess(source);
    assert!(actual.contains("<rect class=\"round\"/>"));
}

#[test]
fn auto_leaves_an_absent_geometry_attribute_absent() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>rect{rx:auto;ry:5px}</style><rect width="20" height="20"/></svg>"#;
    let expected = source.replace("height=\"20\"/>", "height=\"20\" ry=\"5px\"/>");
    assert_eq!(preprocess(source), expected);
}

#[test]
fn interactive_pseudo_classes_do_not_override_static_svg_geometry() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><style>rect { rx: 4px; } rect:hover { rx: 12px; } rect:focus { rx: 15px; }</style><rect x="2" y="2" width="30" height="30"/></svg>"#;
    let actual = preprocess(source);
    assert!(actual.contains("width=\"30\" height=\"30\" rx=\"4px\""));
    assert!(!actual.contains("rx=\"12px\""));
    assert!(!actual.contains("rx=\"15px\""));
}

#[test]
fn unrelated_svg_and_attribute_only_markup_keep_original_bytes() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.shape { fill: red; }</style><rect rx="5"/></svg>"#;
    let without_geometry_css = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect rx="5"/></svg>"#;
    assert_eq!(preprocess(source), source);
    assert_eq!(preprocess(without_geometry_css), without_geometry_css);
}
