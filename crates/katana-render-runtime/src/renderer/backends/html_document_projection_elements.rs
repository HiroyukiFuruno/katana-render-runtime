use super::sibling_positions::{next_sibling_positions, sibling_type_counts};
use super::{HtmlDocument, HtmlDocumentNode, InteractiveElementProjection, ProjectionState};
use crate::renderer::backends::html_css::{HtmlAttributes, StaticCss};
use crate::renderer::backends::html_css_selector::CssAncestor;
use html5ever::Attribute;
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::rc::Rc;

pub(super) fn project_interactive_children(
    document: &HtmlDocument,
    node: &Handle,
    css: &StaticCss,
    ancestors: &[CssAncestor],
    state: ProjectionState<'_>,
) -> Vec<HtmlDocumentNode> {
    let type_counts = sibling_type_counts(node);
    let mut type_indices = HashMap::<String, usize>::new();
    let mut sibling_index = 0;
    node.children
        .borrow()
        .iter()
        .filter_map(|child| {
            let positions =
                next_sibling_positions(child, &mut sibling_index, &mut type_indices, &type_counts);
            document.interactive_node_with_state(child, css, ancestors, positions, state)
        })
        .collect()
}

impl InteractiveElementProjection<'_> {
    pub(super) fn project(self, document: &HtmlDocument) -> Option<HtmlDocumentNode> {
        if super::tags::is_non_rendered_tag(&self.tag) {
            return None;
        }
        let node_id = document
            .node_ids
            .get(&(Rc::as_ptr(self.node) as usize))
            .copied()?;
        let interaction = self.interaction_state(node_id);
        let (attributes, child_ancestors) = self.attributes_and_ancestors(interaction);
        if self.tag == "svg" {
            return Some(self.project_embedded_svg(
                document,
                node_id,
                attributes,
                &child_ancestors,
            ));
        }
        let children = self.children_with_pseudo(document, node_id, interaction, &child_ancestors);
        Some(HtmlDocumentNode::Element {
            node_id,
            tag: self.tag,
            attributes,
            children,
        })
    }

    fn project_embedded_svg(
        &self,
        document: &HtmlDocument,
        node_id: u64,
        attributes: HtmlAttributes,
        ancestors: &[CssAncestor],
    ) -> HtmlDocumentNode {
        super::embedded_svg::embedded_svg_node(
            node_id,
            self.tag.clone(),
            attributes,
            self.node,
            super::embedded_svg::SvgProjectionContext {
                document,
                css: self.css,
                ancestors,
                state: self.state,
            },
        )
    }

    fn interaction_state(&self, node_id: u64) -> ElementInteraction {
        ElementInteraction {
            hovered: self.state.hovered.contains(&node_id),
            focused: self.state.focused.contains(&node_id),
            focus_visible: self.state.focus_visible.contains(&node_id),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ElementInteraction {
    pub(super) hovered: bool,
    pub(super) focused: bool,
    pub(super) focus_visible: bool,
}

pub(in crate::renderer::backends) fn attributes(source: &[Attribute]) -> HtmlAttributes {
    source
        .iter()
        .map(|attribute| {
            (
                attribute.name.local.to_string().to_ascii_lowercase(),
                attribute.value.to_string(),
            )
        })
        .collect()
}
