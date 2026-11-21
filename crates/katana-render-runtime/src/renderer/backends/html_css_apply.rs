use super::super::html_css_cascade::{
    inherited_custom_properties, resolved_css_values, resolved_css_values_and_variable_overrides,
};
use super::super::html_css_rule::CssRule;
use super::super::html_css_rule::CssRuleMatchState;
use super::super::html_css_selector::CssAncestor;
use super::resolution::select_inline_declarations;
use super::{CssResolutionMode, HtmlAttributes, StaticCss};

impl StaticCss {
    pub(in crate::renderer::backends) fn apply(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
    ) -> HtmlAttributes {
        self.apply_with_ancestors(tag, attributes, &[])
    }

    pub(in crate::renderer::backends) fn apply_with_ancestors(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
    ) -> HtmlAttributes {
        self.apply_with_ancestors_at(tag, attributes, ancestors, 1)
    }

    pub(in crate::renderer::backends) fn apply_with_ancestors_at(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
    ) -> HtmlAttributes {
        self.apply_with_ancestors_at_state(tag, attributes, ancestors, sibling_index, false)
    }

    pub(in crate::renderer::backends) fn apply_with_ancestors_at_state(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
        hovered: bool,
    ) -> HtmlAttributes {
        self.apply_with_ancestors_at_state_with_type(CssRuleMatchState::element_with_type(
            tag,
            attributes,
            ancestors,
            sibling_index,
            sibling_index,
            sibling_index,
            hovered,
        ))
    }

    pub(in crate::renderer::backends) fn apply_with_ancestors_at_state_with_type(
        &self,
        state: CssRuleMatchState<'_>,
    ) -> HtmlAttributes {
        let mut rendered = state
            .attributes
            .iter()
            .filter(|(name, _)| !name.eq_ignore_ascii_case("style"))
            .cloned()
            .collect::<HtmlAttributes>();
        let stylesheet = self.resolved_declarations(state);
        if stylesheet.is_empty() {
            return rendered;
        }
        rendered.push(("style".to_string(), stylesheet.join("; ")));
        rendered
    }

    fn resolved_declarations(&self, state: CssRuleMatchState<'_>) -> Vec<String> {
        let Some(selected) = self.selected_declarations(state) else {
            return Vec::new();
        };
        resolved_css_values(selected)
    }

    pub(in crate::renderer::backends) fn apply_with_svg_variable_overrides(
        &self,
        state: CssRuleMatchState<'_>,
    ) -> (HtmlAttributes, Vec<String>) {
        let mut rendered = state
            .attributes
            .iter()
            .filter(|(name, _)| !name.eq_ignore_ascii_case("style"))
            .cloned()
            .collect::<HtmlAttributes>();
        let Some(selected) = self.selected_declarations(state) else {
            return (rendered, Vec::new());
        };
        let (computed, variable_overrides) = resolved_css_values_and_variable_overrides(selected);
        if !computed.is_empty() {
            rendered.push(("style".to_string(), computed.join("; ")));
        }
        (rendered, variable_overrides)
    }

    fn selected_declarations(
        &self,
        state: CssRuleMatchState<'_>,
    ) -> Option<Vec<super::super::html_css_cascade::SelectedDeclaration>> {
        let mut selected = inherited_custom_properties(state.ancestors);
        self.select_rule_declarations(state, &mut selected);
        select_inline_declarations(state.attributes, &mut selected);
        (!selected.is_empty()).then_some(selected)
    }

    pub(in crate::renderer::backends) fn select_rule_declarations(
        &self,
        state: CssRuleMatchState<'_>,
        selected: &mut Vec<super::super::html_css_cascade::SelectedDeclaration>,
    ) {
        for (rule_order, rule) in self.rules.iter().enumerate() {
            let Some(specificity) = self.rule_specificity(rule, state) else {
                continue;
            };
            self.select_matched_declarations(rule, rule_order, specificity, selected);
        }
    }

    fn rule_specificity(&self, rule: &CssRule, state: CssRuleMatchState<'_>) -> Option<u16> {
        match self.mode {
            CssResolutionMode::StaticSnapshot => state
                .pseudo_element
                .is_none()
                .then(|| rule.matches_static_snapshot(state.tag, state.attributes))
                .flatten(),
            CssResolutionMode::InteractiveRuntime => rule.matches_state(state, self.viewport_width),
        }
    }
}
