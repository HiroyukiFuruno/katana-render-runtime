use super::html_css::HtmlAttributes;
use super::html_css_rule::{CssDeclaration, parse_declarations};
use super::html_css_selector::CssAncestor;
use std::collections::{HashMap, HashSet};

const CSS_VARIABLE_FUNCTION_PREFIX: &str = "var(";

#[derive(Debug)]
pub(super) struct SelectedDeclaration {
    name: String,
    value: String,
    specificity: u16,
    rule_order: usize,
    declaration_order: usize,
    important: bool,
    author_origin: bool,
}

pub(super) fn style_attribute(attributes: &HtmlAttributes) -> &str {
    attributes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("style"))
        .map_or("", |(_, value)| value.as_str())
}

pub(super) fn select_declaration(
    selected: &mut Vec<SelectedDeclaration>,
    declaration: &CssDeclaration,
    specificity: u16,
    rule_order: usize,
    declaration_order: usize,
    author_origin: bool,
) {
    let candidate = SelectedDeclaration {
        name: declaration.name.clone(),
        value: declaration.value.clone(),
        specificity,
        rule_order,
        declaration_order,
        important: declaration.important,
        author_origin,
    };
    let Some(current) = selected
        .iter_mut()
        .find(|item| item.name == declaration.name)
    else {
        selected.push(candidate);
        return;
    };
    if candidate.precedence() >= current.precedence() {
        *current = candidate;
    }
}

impl SelectedDeclaration {
    fn precedence(&self) -> (bool, bool, u16, usize, usize) {
        (
            self.important,
            self.author_origin,
            self.specificity,
            self.rule_order,
            self.declaration_order,
        )
    }
}

fn inherited_custom_declarations(style: &str) -> Vec<CssDeclaration> {
    /* WHY: 通常の祖先styleを全要素・疑似要素で再parseせず、escapeされたcustom名は通常parserで保持する。 */
    if !style.contains("--") && !style.contains('\\') {
        return Vec::new();
    }
    parse_declarations(style)
        .into_iter()
        .filter(|declaration| declaration.name.starts_with("--"))
        .collect()
}

pub(super) fn inherited_custom_properties(ancestors: &[CssAncestor]) -> Vec<SelectedDeclaration> {
    let mut selected = Vec::new();
    for (ancestor_order, ancestor) in ancestors.iter().enumerate() {
        for (declaration_order, declaration) in
            inherited_custom_declarations(style_attribute(ancestor.attributes()))
                .iter()
                .enumerate()
        {
            let inherited = CssDeclaration {
                name: declaration.name.clone(),
                value: declaration.value.clone(),
                important: false,
            };
            select_declaration(
                &mut selected,
                &inherited,
                0,
                ancestor_order,
                declaration_order,
                true,
            );
        }
    }
    selected
}

pub(super) fn resolved_css_values(selected: Vec<SelectedDeclaration>) -> Vec<String> {
    let custom = selected
        .iter()
        .filter(|declaration| declaration.name.starts_with("--"))
        .map(|declaration| (declaration.name.clone(), declaration.value.clone()))
        .collect::<HashMap<_, _>>();
    selected
        .into_iter()
        .filter_map(|declaration| {
            let value = if declaration.name.starts_with("--") {
                Some(declaration.value)
            } else {
                resolve_css_variables(&declaration.value, &custom, &mut HashSet::new())
            }?;
            Some(format!("{}: {}", declaration.name, value))
        })
        .collect()
}

pub(super) fn resolve_css_variables(
    value: &str,
    custom: &HashMap<String, String>,
    resolving: &mut HashSet<String>,
) -> Option<String> {
    let mut output = String::new();
    let mut remaining = value;
    while let Some(start) = remaining.find(CSS_VARIABLE_FUNCTION_PREFIX) {
        output.push_str(&remaining[..start]);
        let arguments = &remaining[start + CSS_VARIABLE_FUNCTION_PREFIX.len()..];
        let end = matching_parenthesis(arguments)?;
        let (name, fallback) = split_variable_arguments(&arguments[..end]);
        let name = name.trim().to_string();
        let replacement = resolve_variable_replacement(&name, fallback, custom, resolving)?;
        output.push_str(&replacement);
        remaining = &arguments[end + 1..];
    }
    output.push_str(remaining);
    Some(output)
}

fn resolve_variable_replacement(
    name: &str,
    fallback: Option<&str>,
    custom: &HashMap<String, String>,
    resolving: &mut HashSet<String>,
) -> Option<String> {
    let resolved = if name.starts_with("--") && resolving.insert(name.to_string()) {
        let resolved = custom
            .get(name)
            .and_then(|value| resolve_css_variables(value, custom, resolving));
        resolving.remove(name);
        resolved
    } else {
        None
    };
    resolved.or_else(|| {
        fallback.and_then(|fallback| resolve_css_variables(fallback.trim(), custom, resolving))
    })
}

fn matching_parenthesis(value: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    for (index, character) in value.char_indices() {
        if let Some(expected) = quote {
            if character == expected {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' => depth += 1,
            ')' if depth == 0 => return Some(index),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn split_variable_arguments(arguments: &str) -> (&str, Option<&str>) {
    let mut depth = 0usize;
    for (index, character) in arguments.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return (&arguments[..index], Some(&arguments[index + 1..])),
            _ => {}
        }
    }
    (arguments, None)
}

#[cfg(test)]
mod tests {
    use super::{SelectedDeclaration, resolve_css_variables, resolved_css_values};
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
}

#[cfg(test)]
mod inherited_parse_tests {
    #[test]
    fn inherited_custom_properties_preserve_escaped_names_and_omit_ordinary_styles() {
        assert!(super::inherited_custom_declarations("font-size: 16px; color: green").is_empty());
        let declarations =
            super::inherited_custom_declarations(r"\2d\2d marker: green; color: red");
        assert_eq!(declarations.len(), 1);
        assert_eq!(declarations[0].name, "--marker");
        assert_eq!(declarations[0].value, "green");
    }
}
