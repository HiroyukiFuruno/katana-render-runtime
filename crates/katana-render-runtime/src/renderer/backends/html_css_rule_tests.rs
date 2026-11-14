use super::{media_query_matches, parse_declarations, parse_rules};

#[test]
fn rules_keep_layout_declarations_for_the_interactive_runtime() {
    let rules = parse_rules(
        "main { margin: 24px; padding: 16px; width: 600px; min-height: 400px; font-size: 18px; }",
    );

    assert_eq!(rules.len(), 1);
    let rule = &rules[0];
    let declarations = rule
        .declarations
        .iter()
        .map(|declaration| declaration.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        declarations,
        vec![
            "margin-top",
            "margin-right",
            "margin-bottom",
            "margin-left",
            "padding-top",
            "padding-right",
            "padding-bottom",
            "padding-left",
            "width",
            "min-height",
            "font-size",
        ]
    );
}

#[test]
fn valid_declarations_are_preserved_for_typed_computed_style() {
    let rules = parse_rules("p { display: none; position: fixed; }");

    assert_eq!(rules[0].declarations.len(), 2);
    assert_eq!(rules[0].declarations[0].name, "display");
    assert_eq!(rules[0].declarations[1].name, "position");
}

#[test]
fn parse_box_shorthand_is_expanded_into_longhands() {
    let declarations = parse_declarations(
        "padding: 4px 5px 6px 7px; margin: 10px 20px; margin-top: 30px; padding: 1px 2px 3px 4px 5px;",
    );
    let actual = declarations
        .iter()
        .map(|declaration| (declaration.name.as_str(), declaration.value.as_str()))
        .collect::<Vec<_>>();

    assert_eq!(actual, expected_box_declarations());
}

fn expected_box_declarations() -> Vec<(&'static str, &'static str)> {
    vec![
        ("padding-top", "4px"),
        ("padding-right", "5px"),
        ("padding-bottom", "6px"),
        ("padding-left", "7px"),
        ("margin-top", "10px"),
        ("margin-right", "20px"),
        ("margin-bottom", "10px"),
        ("margin-left", "20px"),
        ("margin-top", "30px"),
        ("padding", "1px 2px 3px 4px 5px"),
    ]
}

#[test]
fn structured_parser_keeps_delimiters_in_strings_functions_and_important() {
    let rules = parse_rules(
        r#".card { --label: "a;b:c}"; color: var(--label); background: rgb(1, 2, 3) !important; }"#,
    );

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].declarations[0].value, r#""a;b:c}""#);
    assert_eq!(rules[0].declarations[1].value, "var(--label)");
    assert!(rules[0].declarations[2].important);
}

#[test]
fn media_query_matches_the_logical_viewport_width() {
    assert!(media_query_matches("screen and (min-width: 600px)", 800.0));
    assert!(!media_query_matches("screen and (max-width: 600px)", 800.0));
    assert!(!media_query_matches("print", 800.0));
}

#[test]
fn media_query_accepts_a_valid_alternative_after_a_nonmatching_one() {
    assert!(media_query_matches(
        "screen and (min-width: 900px), screen and (max-width: 500px)",
        400.0
    ));
    assert!(!media_query_matches(
        "screen and (min-width: 900px), screen and (max-width: 500px)",
        700.0
    ));
}

#[test]
fn nth_child_odd_and_implicit_positive_step_match_real_sibling_positions() {
    let rules =
        parse_rules("li:nth-child(odd) { color: red; } li:nth-child(+n + 3) { color: blue; }");
    const VIEWPORT_WIDTH: f32 = 800.0;
    let attributes = Vec::new();

    assert_eq!(rules.len(), 2);
    for (rule, positions) in rules.iter().zip([
        [(1, true), (2, false), (3, true)],
        [(2, false), (3, true), (7, true)],
    ]) {
        for (position, expected) in positions {
            let matched = rule.matches_at("li", &attributes, &[], position, VIEWPORT_WIDTH);
            assert_eq!(matched.is_some(), expected, "position {position}");
        }
    }
}

#[test]
fn media_query_rules_are_respected_in_interactive_matching() {
    let rules = parse_rules("@media (max-width: 500px) { .card { color: red; } }");
    let selectors = vec![(String::from("class"), String::from("card"))];

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].matches("div", &selectors, &[], 600.0), None);
    assert!(rules[0].matches("div", &selectors, &[], 400.0).is_some());
}
