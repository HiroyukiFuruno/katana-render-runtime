use super::super::html_css::HtmlAttributes;
use super::super::html_css_rule::CssRuleMatchState;
use super::{CssAttributeMatch, CssAttributeSelector, CssCompoundSelector, CssNthExpression};

impl CssCompoundSelector {
    pub(super) fn matches(&self, state: CssRuleMatchState<'_>) -> bool {
        let disabled = attribute_value(state.attributes, "disabled").is_some();
        let checked = attribute_value(state.attributes, "checked").is_some();
        self.matches_identity(state.tag, state.attributes)
            && self.matches_state(state, disabled, checked)
            && self
                .nth_child
                .is_none_or(|expression| expression.matches(state.sibling_index))
            && (!self.first_of_type || state.sibling_type_index == 1)
            && (!self.last_of_type || state.sibling_type_index == state.sibling_type_count)
            && (!self.only_of_type || state.sibling_type_count == 1)
            && (!self.first_child || state.sibling_index == 1)
            && (!self.last_child || state.sibling_index == state.sibling_type_count)
            && (!self.only_child || state.sibling_type_count == 1)
    }

    fn matches_identity(&self, tag: &str, attributes: &HtmlAttributes) -> bool {
        self.tag
            .as_ref()
            .is_none_or(|name| name.eq_ignore_ascii_case(tag))
            && self
                .classes
                .iter()
                .all(|class| has_class(attributes, class))
            && self
                .id
                .as_ref()
                .is_none_or(|id| attribute_value(attributes, "id").is_some_and(|value| value == id))
            && self
                .attributes
                .iter()
                .all(|selector| selector.matches(attributes))
            && (!self.root || tag.eq_ignore_ascii_case("html"))
    }

    fn matches_state(&self, state: CssRuleMatchState<'_>, disabled: bool, checked: bool) -> bool {
        (!self.hovered || state.hovered)
            && (!self.focused || state.focused)
            && (!self.focus_visible || state.focus_visible)
            && (!self.disabled || disabled)
            && (!self.not_disabled || !disabled)
            && (!self.checked || checked)
    }
}

impl CssNthExpression {
    pub(super) fn matches(self, sibling_index: usize) -> bool {
        let Ok(index) = i32::try_from(sibling_index) else {
            return false;
        };
        if index < 1 {
            return false;
        }
        if self.step == 0 {
            return index == self.offset;
        }
        let delta = index - self.offset;
        delta % self.step == 0 && delta / self.step >= 0
    }
}

impl CssAttributeSelector {
    fn matches(&self, attributes: &HtmlAttributes) -> bool {
        attribute_value(attributes, &self.name).is_some_and(|candidate| match &self.matching {
            CssAttributeMatch::Exists => true,
            CssAttributeMatch::Equals(expected) => candidate == expected,
            CssAttributeMatch::Prefix(expected) => {
                !expected.is_empty() && candidate.starts_with(expected)
            }
        })
    }
}

fn has_class(attributes: &HtmlAttributes, class: &str) -> bool {
    attribute_value(attributes, "class").is_some_and(|classes| {
        classes
            .split_whitespace()
            .any(|candidate| candidate == class)
    })
}

fn attribute_value<'a>(attributes: &'a HtmlAttributes, name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}
