use super::super::sibling_positions::{next_sibling_positions, sibling_type_counts};
use super::super::{HtmlAttributes, HtmlDocument, ProjectionState};
use crate::renderer::backends::html_css::StaticCss;
use crate::renderer::backends::html_css_rule::CssRuleMatchState;
use crate::renderer::backends::html_css_selector::{CssAncestor, CssInteractionState};
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;
use std::rc::Rc;

struct SvgElementContext {
    tag: String,
    attributes: HtmlAttributes,
    positions: (usize, usize, usize),
    interaction: CssInteractionState,
}

pub(super) fn svg_variable_overrides(
    document: &HtmlDocument,
    root: &Handle,
    css: &StaticCss,
    ancestors: &[CssAncestor],
    state: ProjectionState<'_>,
) -> HashMap<usize, Vec<String>> {
    if !css.has_variable_declarations() && !svg_has_inline_variables(root) {
        return HashMap::new();
    }
    let mut overrides = HashMap::new();
    collect_svg_variable_overrides(document, root, css, ancestors, state, &mut overrides);
    overrides
}

fn svg_has_inline_variables(node: &Handle) -> bool {
    let NodeData::Element { attrs, .. } = &node.data else {
        return node.children.borrow().iter().any(svg_has_inline_variables);
    };
    let has_variables = attrs.borrow().iter().any(|attribute| {
        attribute.name.local.as_str().eq_ignore_ascii_case("style")
            && attribute.value.contains("var(")
    });
    has_variables || node.children.borrow().iter().any(svg_has_inline_variables)
}

fn collect_svg_variable_overrides(
    document: &HtmlDocument,
    parent: &Handle,
    css: &StaticCss,
    ancestors: &[CssAncestor],
    state: ProjectionState<'_>,
    overrides: &mut HashMap<usize, Vec<String>>,
) {
    let counts = sibling_type_counts(parent);
    let mut indices = HashMap::new();
    let mut sibling_index = 0;
    for child in parent.children.borrow().iter() {
        let positions = next_sibling_positions(child, &mut sibling_index, &mut indices, &counts);
        collect_svg_element_override(document, child, css, ancestors, state, positions, overrides);
    }
}

fn collect_svg_element_override(
    document: &HtmlDocument,
    node: &Handle,
    css: &StaticCss,
    ancestors: &[CssAncestor],
    state: ProjectionState<'_>,
    positions: (usize, usize, usize),
    overrides: &mut HashMap<usize, Vec<String>>,
) {
    let Some((pointer, context, variable_overrides)) =
        projected_svg_element(document, node, css, ancestors, state, positions)
    else {
        return;
    };
    if !variable_overrides.is_empty() {
        overrides.insert(pointer, variable_overrides);
    }
    collect_svg_child_overrides(document, node, css, ancestors, state, overrides, context);
}

fn collect_svg_child_overrides(
    document: &HtmlDocument,
    node: &Handle,
    css: &StaticCss,
    ancestors: &[CssAncestor],
    state: ProjectionState<'_>,
    overrides: &mut HashMap<usize, Vec<String>>,
    context: SvgElementContext,
) {
    let children = svg_child_ancestors(ancestors, &context);
    collect_svg_variable_overrides(document, node, css, &children, state, overrides);
}

fn projected_svg_element(
    document: &HtmlDocument,
    node: &Handle,
    css: &StaticCss,
    ancestors: &[CssAncestor],
    state: ProjectionState<'_>,
    positions: (usize, usize, usize),
) -> Option<(usize, SvgElementContext, Vec<String>)> {
    let (tag, attributes, pointer) = svg_element_source(node)?;
    let interaction = svg_interaction_state(document, pointer, state);
    let state = CssRuleMatchState::element_with_interaction_state(
        &tag,
        &attributes,
        ancestors,
        positions.0,
        positions.1,
        positions.2,
        interaction,
    );
    let (attributes, overrides) = css.apply_with_svg_variable_overrides(state);
    let context = SvgElementContext {
        tag,
        attributes,
        positions,
        interaction,
    };
    Some((pointer, context, overrides))
}

fn svg_element_source(node: &Handle) -> Option<(String, HtmlAttributes, usize)> {
    let NodeData::Element { name, attrs, .. } = &node.data else {
        return None;
    };
    Some((
        name.local.to_string().to_ascii_lowercase(),
        super::super::attributes(&attrs.borrow()),
        Rc::as_ptr(node) as usize,
    ))
}

fn svg_interaction_state(
    document: &HtmlDocument,
    pointer: usize,
    state: ProjectionState<'_>,
) -> CssInteractionState {
    document
        .node_ids
        .get(&pointer)
        .copied()
        .map_or(CssInteractionState::default(), |node_id| {
            CssInteractionState {
                hovered: state.hovered.contains(&node_id),
                focused: state.focused.contains(&node_id),
                focus_visible: state.focus_visible.contains(&node_id),
            }
        })
}

fn svg_child_ancestors(ancestors: &[CssAncestor], context: &SvgElementContext) -> Vec<CssAncestor> {
    let mut result = ancestors.to_vec();
    result.push(CssAncestor::new_with_interaction_state(
        &context.tag,
        &context.attributes,
        context.positions.0,
        context.positions.1,
        context.positions.2,
        context.interaction,
    ));
    result
}
