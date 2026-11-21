use super::{CssAncestor, CssInteractionState, HtmlAttributes};

impl CssAncestor {
    #[cfg(test)]
    pub(in super::super) fn new(tag: &str, attributes: &HtmlAttributes) -> Self {
        Self::new_at(tag, attributes, 1)
    }

    #[cfg(test)]
    pub(in super::super) fn new_at(
        tag: &str,
        attributes: &HtmlAttributes,
        sibling_index: usize,
    ) -> Self {
        Self::new_at_state(tag, attributes, sibling_index, false)
    }

    #[cfg(test)]
    pub(in super::super) fn new_at_state(
        tag: &str,
        attributes: &HtmlAttributes,
        sibling_index: usize,
        hovered: bool,
    ) -> Self {
        Self::new_at_state_with_type(
            tag,
            attributes,
            sibling_index,
            sibling_index,
            sibling_index,
            hovered,
        )
    }

    pub(in super::super) fn new_at_state_with_type(
        tag: &str,
        attributes: &HtmlAttributes,
        sibling_index: usize,
        sibling_type_index: usize,
        sibling_type_count: usize,
        hovered: bool,
    ) -> Self {
        Self {
            tag: tag.to_string(),
            attributes: attributes.clone(),
            sibling_index,
            sibling_type_index,
            sibling_type_count,
            hovered,
            focused: false,
            focus_visible: false,
        }
    }

    pub(in super::super) fn new_with_interaction_state(
        tag: &str,
        attributes: &HtmlAttributes,
        sibling_index: usize,
        sibling_type_index: usize,
        sibling_type_count: usize,
        state: CssInteractionState,
    ) -> Self {
        Self {
            tag: tag.to_string(),
            attributes: attributes.clone(),
            sibling_index,
            sibling_type_index,
            sibling_type_count,
            hovered: state.hovered,
            focused: state.focused,
            focus_visible: state.focus_visible,
        }
    }

    pub(in super::super) fn focused(&self) -> bool {
        self.focused
    }

    pub(in super::super) fn focus_visible(&self) -> bool {
        self.focus_visible
    }

    pub(in super::super) fn attributes(&self) -> &HtmlAttributes {
        &self.attributes
    }
}
