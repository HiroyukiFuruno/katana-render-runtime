use super::super::html_css_cascade::{inherited_custom_properties, resolved_css_values};
use super::super::html_css_rule::CssRuleMatchState;
use super::super::html_css_selector::CssInteractionState;
use super::generated_content::{CssPseudoStyle, parse_generated_content};
use super::{CssPseudoRequest, CssResolutionMode, StaticCss};

impl StaticCss {
    pub(in crate::renderer::backends) fn pseudo_style_at_state(
        &self,
        request: CssPseudoRequest<'_>,
    ) -> Option<CssPseudoStyle> {
        if self.mode != CssResolutionMode::InteractiveRuntime {
            return None;
        }
        let mut selected = inherited_custom_properties(request.inheritance_ancestors);
        let state = pseudo_rule_state(request);
        self.select_rule_declarations(state, &mut selected);
        let declarations = resolved_css_values(selected);
        let content = declarations
            .iter()
            .find_map(|declaration| declaration.strip_prefix("content: "))
            .and_then(parse_generated_content)?;
        Some(CssPseudoStyle {
            attributes: vec![("style".to_string(), declarations.join("; "))],
            content,
        })
    }
}

fn pseudo_rule_state(request: CssPseudoRequest<'_>) -> CssRuleMatchState<'_> {
    CssRuleMatchState::element_with_interaction_state(
        request.tag,
        request.attributes,
        request.ancestors,
        request.sibling_index,
        request.sibling_type_index,
        request.sibling_type_count,
        CssInteractionState {
            hovered: request.hovered,
            focused: request.focused,
            focus_visible: request.focus_visible,
        },
    )
    .for_pseudo(request.pseudo_element)
}
