use super::SelectedDeclaration;
use std::collections::{HashMap, HashSet};

const CSS_VARIABLE_FUNCTION_PREFIX: &str = "var(";

pub(in crate::renderer::backends) fn resolved_css_values(
    selected: Vec<SelectedDeclaration>,
) -> Vec<String> {
    resolve_selected_css_values(selected, false).0
}

pub(in crate::renderer::backends) fn resolved_css_values_and_variable_overrides(
    selected: Vec<SelectedDeclaration>,
) -> (Vec<String>, Vec<String>) {
    resolve_selected_css_values(selected, true)
}

fn resolve_selected_css_values(
    selected: Vec<SelectedDeclaration>,
    collect_variable_overrides: bool,
) -> (Vec<String>, Vec<String>) {
    let custom = selected
        .iter()
        .filter(|declaration| declaration.name.starts_with("--"))
        .map(|declaration| (declaration.name.clone(), declaration.value.clone()))
        .collect::<HashMap<_, _>>();
    let mut resolved = Vec::new();
    let mut variable_overrides = Vec::new();
    for declaration in selected {
        if let Some((computed, variable_override)) =
            resolved_declaration(declaration, &custom, collect_variable_overrides)
        {
            resolved.push(computed);
            variable_overrides.extend(variable_override);
        }
    }
    (resolved, variable_overrides)
}

fn resolved_declaration(
    declaration: SelectedDeclaration,
    custom: &HashMap<String, String>,
    collect_variable_override: bool,
) -> Option<(String, Option<String>)> {
    let custom_property = declaration.name.starts_with("--");
    let has_variable = !custom_property
        && collect_variable_override
        && declaration.value.contains(CSS_VARIABLE_FUNCTION_PREFIX);
    let value = if custom_property {
        Some(declaration.value)
    } else {
        resolve_css_variables(&declaration.value, custom, &mut HashSet::new())
    }?;
    let computed = format!("{}: {}", declaration.name, value);
    let override_value = has_variable.then(|| {
        let important = if declaration.important {
            " !important"
        } else {
            ""
        };
        format!("{}: {}{}", declaration.name, value, important)
    });
    Some((computed, override_value))
}

pub(in crate::renderer::backends) fn resolve_css_variables(
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
#[path = "html_css_variables_tests.rs"]
mod tests;
