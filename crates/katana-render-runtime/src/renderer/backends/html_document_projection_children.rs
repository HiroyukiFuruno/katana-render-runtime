use super::super::super::html_css::HtmlAttributes;
use super::super::super::html_css_rule::CssRuleMatchState;
use super::super::super::html_css_selector::{CssAncestor, CssInteractionState, CssPseudoElement};
use super::elements::ElementInteraction;
use super::{HtmlDocument, HtmlDocumentNode, InteractiveElementProjection};

impl InteractiveElementProjection<'_> {
    pub(super) fn children_with_pseudo(
        &self,
        document: &HtmlDocument,
        node_id: u64,
        interaction: ElementInteraction,
        child_ancestors: &[CssAncestor],
    ) -> Vec<HtmlDocumentNode> {
        let mut children =
            document.interactive_children(self.node, self.css, child_ancestors, self.state);
        self.add_pseudo_children(node_id, interaction, child_ancestors, &mut children);
        children
    }

    fn add_pseudo_children(
        &self,
        node_id: u64,
        interaction: ElementInteraction,
        child_ancestors: &[CssAncestor],
        children: &mut Vec<HtmlDocumentNode>,
    ) {
        for (pseudo, prepend) in [
            (CssPseudoElement::Before, true),
            (CssPseudoElement::After, false),
        ] {
            if let Some(node) = self.pseudo_node(
                node_id,
                pseudo,
                interaction.hovered,
                interaction.focused,
                interaction.focus_visible,
                child_ancestors,
            ) {
                if prepend {
                    children.insert(0, node);
                } else {
                    children.push(node);
                }
            }
        }
    }

    pub(super) fn attributes_and_ancestors(
        &self,
        interaction: ElementInteraction,
    ) -> (HtmlAttributes, Vec<CssAncestor>) {
        let attributes = self
            .css
            .apply_with_ancestors_at_state_with_type(self.rule_match_state(interaction));
        let mut child_ancestors = self.ancestors.to_vec();
        child_ancestors.push(CssAncestor::new_with_interaction_state(
            &self.tag,
            &attributes,
            self.sibling_index,
            self.sibling_type_index,
            self.sibling_type_count,
            self.css_interaction_state(interaction),
        ));
        (attributes, child_ancestors)
    }

    fn rule_match_state(&self, interaction: ElementInteraction) -> CssRuleMatchState<'_> {
        CssRuleMatchState::element_with_interaction_state(
            &self.tag,
            &self.source_attributes,
            self.ancestors,
            self.sibling_index,
            self.sibling_type_index,
            self.sibling_type_count,
            self.css_interaction_state(interaction),
        )
    }

    fn css_interaction_state(&self, interaction: ElementInteraction) -> CssInteractionState {
        CssInteractionState {
            hovered: interaction.hovered,
            focused: interaction.focused,
            focus_visible: interaction.focus_visible,
        }
    }
}
