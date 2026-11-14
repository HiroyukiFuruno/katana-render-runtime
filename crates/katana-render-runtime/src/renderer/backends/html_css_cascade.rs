use super::html_css::HtmlAttributes;
use super::html_css_rule::{CssDeclaration, parse_declarations};
use super::html_css_selector::CssAncestor;

#[path = "html_css_variables.rs"]
mod variables;

#[cfg(test)]
pub(in crate::renderer::backends) use variables::resolve_css_variables;
pub(in crate::renderer::backends) use variables::{
    resolved_css_values, resolved_css_values_and_variable_overrides,
};

#[derive(Debug)]
pub(super) struct SelectedDeclaration {
    pub(super) name: String,
    pub(super) value: String,
    pub(super) specificity: u16,
    pub(super) rule_order: usize,
    pub(super) declaration_order: usize,
    pub(super) important: bool,
    pub(super) author_origin: bool,
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
