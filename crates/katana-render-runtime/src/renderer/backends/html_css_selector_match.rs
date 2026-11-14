use super::super::html_css_rule::CssRuleMatchState;
use super::{CssAncestor, CssCombinator, CssInteractionState, CssSelector, HtmlAttributes};

impl CssSelector {
    pub(in crate::renderer::backends) fn matches(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
    ) -> bool {
        self.matches_at(tag, attributes, ancestors, 1)
    }

    pub(in crate::renderer::backends) fn matches_at(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
    ) -> bool {
        self.matches_at_state_with_type(CssRuleMatchState::element_with_type(
            tag,
            attributes,
            ancestors,
            sibling_index,
            sibling_index,
            sibling_index,
            false,
        ))
    }

    pub(in crate::renderer::backends) fn matches_at_with_type(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
        sibling_type_index: usize,
        sibling_type_count: usize,
    ) -> bool {
        self.matches_at_state_with_type(CssRuleMatchState::element_with_type(
            tag,
            attributes,
            ancestors,
            sibling_index,
            sibling_type_index,
            sibling_type_count,
            false,
        ))
    }

    #[cfg(test)]
    pub(in crate::renderer::backends) fn matches_at_state(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
        hovered: bool,
    ) -> bool {
        self.matches_at_state_with_type(CssRuleMatchState::element_with_type(
            tag,
            attributes,
            ancestors,
            sibling_index,
            sibling_index,
            sibling_index,
            hovered,
        ))
    }

    #[cfg(test)]
    pub(in crate::renderer::backends) fn matches_at_focus_state(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        focused: bool,
        focus_visible: bool,
    ) -> bool {
        self.matches_at_state_with_type(CssRuleMatchState::element_with_interaction_state(
            tag,
            attributes,
            ancestors,
            1,
            1,
            1,
            CssInteractionState {
                focused,
                focus_visible,
                ..CssInteractionState::default()
            },
        ))
    }

    pub(in crate::renderer::backends) fn matches_at_state_with_type(
        &self,
        state: CssRuleMatchState<'_>,
    ) -> bool {
        self.matches_at_pseudo_state(state)
    }

    pub(in crate::renderer::backends) fn matches_at_pseudo_state(
        &self,
        state: CssRuleMatchState<'_>,
    ) -> bool {
        if self.pseudo_element() != state.pseudo_element {
            return false;
        }
        self.matches_from(self.compounds.len() - 1, state)
    }

    fn matches_from(&self, index: usize, state: CssRuleMatchState<'_>) -> bool {
        if !self.compounds[index].matches(state) {
            return false;
        }
        if index == 0 {
            return true;
        }
        match self.combinators[index - 1] {
            CssCombinator::Child => self.matches_parent(index, state.ancestors),
            CssCombinator::Descendant => self.matches_ancestor(index, state.ancestors),
        }
    }

    fn matches_parent(&self, index: usize, ancestors: &[CssAncestor]) -> bool {
        ancestors.last().is_some_and(|parent| {
            self.matches_from(
                index - 1,
                CssRuleMatchState::element_with_interaction_state(
                    &parent.tag,
                    &parent.attributes,
                    &ancestors[..ancestors.len() - 1],
                    parent.sibling_index,
                    parent.sibling_type_index,
                    parent.sibling_type_count,
                    CssInteractionState {
                        hovered: parent.hovered,
                        focused: parent.focused(),
                        focus_visible: parent.focus_visible(),
                    },
                ),
            )
        })
    }

    fn matches_ancestor(&self, index: usize, ancestors: &[CssAncestor]) -> bool {
        ancestors.iter().enumerate().rev().any(|(position, item)| {
            self.matches_from(
                index - 1,
                CssRuleMatchState::element_with_interaction_state(
                    &item.tag,
                    &item.attributes,
                    &ancestors[..position],
                    item.sibling_index,
                    item.sibling_type_index,
                    item.sibling_type_count,
                    CssInteractionState {
                        hovered: item.hovered,
                        focused: item.focused(),
                        focus_visible: item.focus_visible(),
                    },
                ),
            )
        })
    }
}
