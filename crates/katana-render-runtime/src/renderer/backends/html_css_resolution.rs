use super::super::html_css_cascade::{SelectedDeclaration, select_declaration, style_attribute};
use super::super::html_css_rule::{CssRule, parse_declarations};
use super::{CssResolutionMode, HtmlAttributes, StaticCss};

pub(super) fn select_inline_declarations(
    attributes: &HtmlAttributes,
    selected: &mut Vec<SelectedDeclaration>,
) {
    for (declaration_order, declaration) in parse_declarations(style_attribute(attributes))
        .iter()
        .enumerate()
    {
        select_declaration(
            selected,
            declaration,
            u16::MAX,
            usize::MAX,
            declaration_order,
            true,
        );
    }
}

impl StaticCss {
    pub(super) fn select_matched_declarations(
        &self,
        rule: &CssRule,
        rule_order: usize,
        specificity: u16,
        selected: &mut Vec<SelectedDeclaration>,
    ) {
        for (declaration_order, declaration) in rule.declarations.iter().enumerate() {
            if self.mode == CssResolutionMode::StaticSnapshot
                && !static_snapshot_property(&declaration.name)
            {
                continue;
            }
            select_declaration(
                selected,
                declaration,
                specificity,
                rule_order,
                declaration_order,
                !rule.user_agent,
            );
        }
    }
}

fn static_snapshot_property(name: &str) -> bool {
    matches!(
        name,
        "background"
            | "background-color"
            | "color"
            | "font-family"
            | "font-style"
            | "font-weight"
            | "font-size"
            | "line-height"
            | "text-align"
            | "text-decoration"
    )
}
