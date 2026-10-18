use super::super::html_css::{HtmlAttributes, StaticCss};
use super::super::html_css_selector::CssAncestor;
use super::{HtmlDocument, HtmlDocumentNode};
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::{HashMap, HashSet};

#[path = "html_document_projection_children.rs"]
mod children;
#[path = "html_document_projection_elements.rs"]
mod elements;
#[path = "html_document_projection_svg.rs"]
mod embedded_svg;
#[path = "html_document_projection_generated.rs"]
mod generated;
#[path = "html_document_projection_siblings.rs"]
mod sibling_positions;
#[path = "html_document_projection_tags.rs"]
mod tags;

pub(in crate::renderer::backends) use elements::attributes;

struct InteractiveElementProjection<'a> {
    node: &'a Handle,
    tag: String,
    source_attributes: HtmlAttributes,
    css: &'a StaticCss,
    ancestors: &'a [CssAncestor],
    sibling_index: usize,
    sibling_type_index: usize,
    sibling_type_count: usize,
    state: ProjectionState<'a>,
}

#[derive(Clone, Copy)]
struct ProjectionState<'a> {
    hovered: &'a HashSet<u64>,
    focused: &'a HashSet<u64>,
    focus_visible: &'a HashSet<u64>,
}

impl HtmlDocument {
    #[cfg(test)]
    pub(in crate::renderer::backends) fn interactive_nodes_with_styles(
        &self,
        external_stylesheets: &HashMap<String, String>,
    ) -> Vec<HtmlDocumentNode> {
        self.interactive_nodes_with_styles_at_width(external_stylesheets, 1024.0)
    }

    #[cfg(test)]
    pub(in crate::renderer::backends) fn interactive_nodes_with_styles_at_width(
        &self,
        external_stylesheets: &HashMap<String, String>,
        viewport_width: f32,
    ) -> Vec<HtmlDocumentNode> {
        self.interactive_nodes_with_styles_at_width_and_hover(
            external_stylesheets,
            viewport_width,
            &HashSet::new(),
        )
    }

    pub(in crate::renderer::backends) fn interactive_nodes_with_styles_at_width_and_hover(
        &self,
        external_stylesheets: &HashMap<String, String>,
        viewport_width: f32,
        hovered_nodes: &HashSet<u64>,
    ) -> Vec<HtmlDocumentNode> {
        self.interactive_nodes_with_styles_at_width_and_state(
            external_stylesheets,
            viewport_width,
            ProjectionState {
                hovered: hovered_nodes,
                focused: &HashSet::new(),
                focus_visible: &HashSet::new(),
            },
        )
    }

    pub(in crate::renderer::backends) fn interactive_nodes_with_styles_at_width_and_focus(
        &self,
        external_stylesheets: &HashMap<String, String>,
        viewport_width: f32,
        hovered_nodes: &HashSet<u64>,
        focused_nodes: &HashSet<u64>,
        focus_visible_nodes: &HashSet<u64>,
    ) -> Vec<HtmlDocumentNode> {
        self.interactive_nodes_with_styles_at_width_and_state(
            external_stylesheets,
            viewport_width,
            ProjectionState {
                hovered: hovered_nodes,
                focused: focused_nodes,
                focus_visible: focus_visible_nodes,
            },
        )
    }

    fn interactive_nodes_with_styles_at_width_and_state(
        &self,
        external_stylesheets: &HashMap<String, String>,
        viewport_width: f32,
        state: ProjectionState<'_>,
    ) -> Vec<HtmlDocumentNode> {
        let css = StaticCss::for_interactive_document_with_styles_at_width(
            &self.document,
            external_stylesheets,
            viewport_width,
        );
        self.interactive_children(&self.document, &css, &[], state)
    }

    fn interactive_children(
        &self,
        node: &Handle,
        css: &StaticCss,
        ancestors: &[CssAncestor],
        state: ProjectionState<'_>,
    ) -> Vec<HtmlDocumentNode> {
        elements::project_interactive_children(self, node, css, ancestors, state)
    }

    fn interactive_node_with_state(
        &self,
        node: &Handle,
        css: &StaticCss,
        ancestors: &[CssAncestor],
        sibling_positions: (usize, usize, usize),
        state: ProjectionState<'_>,
    ) -> Option<HtmlDocumentNode> {
        match &node.data {
            NodeData::Text { contents } => {
                let text = contents.borrow().to_string();
                (!text.is_empty()).then_some(HtmlDocumentNode::Text(text))
            }
            NodeData::Element { .. } => {
                self.project_element_node(node, css, ancestors, sibling_positions, state)
            }
            NodeData::Document => None,
            _ => None,
        }
    }

    fn project_element_node(
        &self,
        node: &Handle,
        css: &StaticCss,
        ancestors: &[CssAncestor],
        sibling_positions: (usize, usize, usize),
        state: ProjectionState<'_>,
    ) -> Option<HtmlDocumentNode> {
        let NodeData::Element { name, attrs, .. } = &node.data else {
            return None;
        };
        let (sibling_index, sibling_type_index, sibling_type_count) = sibling_positions;
        InteractiveElementProjection {
            node,
            tag: name.local.to_string().to_ascii_lowercase(),
            source_attributes: attributes(&attrs.borrow()),
            css,
            ancestors,
            sibling_index,
            sibling_type_index,
            sibling_type_count,
            state,
        }
        .project(self)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{HtmlDocument, HtmlDocumentNode};
    use super::ProjectionState;
    use crate::renderer::backends::html_css::StaticCss;
    use markup5ever_rcdom::{Handle, NodeData};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn interactive_projection_keeps_rendered_children_and_drops_non_rendered_tags() {
        let document = HtmlDocument::parse(
            "<main><p id=target>Text</p><script>ignored()</script><template>hidden</template></main>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        assert!(contains_tag(&nodes, "p"));
        assert!(!contains_tag(&nodes, "script"));
        assert!(!contains_tag(&nodes, "template"));
    }

    #[test]
    fn element_projection_guard_rejects_a_text_handle() -> Result<(), String> {
        let document = HtmlDocument::parse("<main>text</main>");
        let text = find_text(&document.document).ok_or("text node")?;
        let css = StaticCss::for_interactive_document_with_styles_at_width(
            &document.document,
            &HashMap::new(),
            1024.0,
        );
        let hovered = HashSet::new();
        let focused = HashSet::new();
        let focus_visible = HashSet::new();
        assert!(
            document
                .project_element_node(
                    &text,
                    &css,
                    &[],
                    (1, 1, 1),
                    ProjectionState {
                        hovered: &hovered,
                        focused: &focused,
                        focus_visible: &focus_visible,
                    },
                )
                .is_none()
        );
        Ok(())
    }

    fn find_text(node: &Handle) -> Option<Handle> {
        if matches!(node.data, NodeData::Text { .. }) {
            return Some(node.clone());
        }
        node.children.borrow().iter().find_map(find_text)
    }

    fn contains_tag(nodes: &[HtmlDocumentNode], expected: &str) -> bool {
        nodes.iter().any(|node| match node {
            HtmlDocumentNode::Element { tag, children, .. } => {
                tag == expected || contains_tag(children, expected)
            }
            HtmlDocumentNode::Text(_) => false,
        })
    }
}
