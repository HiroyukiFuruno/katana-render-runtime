use super::super::SelectedDeclaration;
use super::{resolve_css_variables, resolved_css_values};
use std::collections::{HashMap, HashSet};

#[test]
fn resolved_css_values_keeps_custom_properties_and_resolves_variable_reference() {
    let selected = vec![
        SelectedDeclaration {
            name: "--accent".to_string(),
            value: "blue".to_string(),
            specificity: 0,
            rule_order: 0,
            declaration_order: 0,
            important: false,
            author_origin: true,
        },
        SelectedDeclaration {
            name: "color".to_string(),
            value: "var(--accent)".to_string(),
            specificity: 0,
            rule_order: 0,
            declaration_order: 1,
            important: false,
            author_origin: true,
        },
    ];

    let values = resolved_css_values(selected);
    assert!(values.contains(&"--accent: blue".to_string()));
    assert!(values.contains(&"color: blue".to_string()));
}

#[test]
fn resolved_css_values_drops_a_property_with_an_unresolved_cycle() {
    let selected = vec![
        SelectedDeclaration {
            name: "--cycle".to_string(),
            value: "var(--cycle)".to_string(),
            specificity: 0,
            rule_order: 0,
            declaration_order: 0,
            important: false,
            author_origin: true,
        },
        SelectedDeclaration {
            name: "color".to_string(),
            value: "var(--cycle)".to_string(),
            specificity: 0,
            rule_order: 0,
            declaration_order: 1,
            important: false,
            author_origin: true,
        },
    ];

    let values = resolved_css_values(selected);
    assert_eq!(values, vec!["--cycle: var(--cycle)".to_string()]);
}

#[test]
fn resolve_css_variables_parses_nested_parentheses_and_fallback() {
    let custom = HashMap::from([
        ("--base".to_string(), "calc(10px + 2px)".to_string()),
        ("--tone".to_string(), "var(--base)".to_string()),
    ]);

    assert_eq!(
        resolve_css_variables("var(--tone, fallback)", &custom, &mut HashSet::new()),
        Some("calc(10px + 2px)".to_string())
    );
}

#[test]
fn resolve_css_variables_returns_none_when_parentheses_missing() {
    let value = "var(--missing";
    assert_eq!(
        resolve_css_variables(value, &HashMap::new(), &mut HashSet::new()),
        None
    );
}

#[test]
fn resolve_css_variables_splits_variable_arguments_for_nested_calls() {
    let custom = HashMap::new();
    let result = resolve_css_variables(
        "var(--missing, fallback(--x))",
        &custom,
        &mut HashSet::new(),
    );
    assert!(result.is_some());

    assert_eq!(result.unwrap_or_default(), "fallback(--x)");
    assert_eq!(
        super::split_variable_arguments(" --missing, fallback(--x)").0,
        " --missing"
    );
}

#[test]
fn resolve_css_variables_handles_quoted_parentheses_and_nested_names() {
    assert_eq!(
        resolve_css_variables(
            r#"var(--missing, "a)b")"#,
            &HashMap::new(),
            &mut HashSet::new()
        ),
        Some(r#""a)b""#.to_string())
    );
    assert_eq!(
        resolve_css_variables(
            "var(var(--nested), fallback)",
            &HashMap::new(),
            &mut HashSet::new()
        ),
        Some("fallback".to_string())
    );
}
