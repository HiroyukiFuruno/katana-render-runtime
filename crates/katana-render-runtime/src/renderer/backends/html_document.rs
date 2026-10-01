use std::collections::HashMap;

use markup5ever_rcdom::Handle;

#[path = "html_document_mutation.rs"]
mod mutation;
#[path = "html_document_projection.rs"]
mod projection;
use projection::attributes;
#[path = "html_document_parse.rs"]
mod parse;
#[path = "html_document_query.rs"]
mod query;
#[path = "html_document_selector.rs"]
mod selector;
#[path = "html_document_svg.rs"]
mod svg;
#[path = "html_document_tree.rs"]
mod tree;

pub(super) use svg::{
    EMBEDDED_SVG_HEIGHT_PLACEHOLDER, EMBEDDED_SVG_MARKUP_ATTRIBUTE, EMBEDDED_SVG_WIDTH_PLACEHOLDER,
    EMBEDDED_SVG_X_PLACEHOLDER, EMBEDDED_SVG_Y_PLACEHOLDER,
};

/// Canonical HTML5 document state shared by CSS rendering and the V8 bridge.
pub(super) struct HtmlDocument {
    pub(super) document: Handle,
    pub(super) body_onload_script_index: Option<usize>,
    pub(super) source_order: Vec<Handle>,
    pub(super) body_onload_source_order_index: Option<usize>,
    nodes: HashMap<u64, Handle>,
    node_ids: HashMap<usize, u64>,
    next_node_id: u64,
}

/// Dynamic DOM projection used only by KRR's interactive HTML runtime.
///
/// It deliberately retains node IDs so the runtime can perform hit-testing and
/// dispatch input without exposing DOM details to KDV or KatanA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum HtmlDocumentNode {
    Element {
        node_id: u64,
        tag: String,
        attributes: super::html_css::HtmlAttributes,
        children: Vec<HtmlDocumentNode>,
    },
    Text(String),
}

#[cfg(test)]
mod tests {
    use super::super::html_css::StaticCss;
    use super::{EMBEDDED_SVG_MARKUP_ATTRIBUTE, HtmlDocument, HtmlDocumentNode};
    use std::collections::HashMap;

    #[test]
    fn document_root_is_never_projected_as_an_interactive_node() {
        let document = HtmlDocument::parse("<p>Visible</p>");
        let css =
            StaticCss::for_interactive_document_with_styles(&document.document, &HashMap::new());

        assert_eq!(
            document.interactive_node(
                &document.document,
                &css,
                &[],
                1,
                &std::collections::HashSet::new(),
            ),
            None
        );
    }

    #[test]
    fn embedded_svg_projection_preserves_vector_markup_and_attribute_case() {
        let document = HtmlDocument::parse(
            r##"<style>svg { max-width: 90px; }</style><svg viewBox="0 0 120 80" preserveAspectRatio="xMidYMid meet" xmlns:xlink="http://www.w3.org/1999/xlink"><!-- marker --><defs><marker id="dot" markerHeight="8"><path d="M0 0L8 4L0 8Z"/></marker></defs><use xlink:href="#dot"/><circle cx="40" cy="40" r="20" fill="#22c55e"/></svg>"##,
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let markup = must_some(
            embedded_svg_markup(&nodes),
            "embedded SVG markup must exist",
        );

        assert!(markup.contains("viewBox=\"0 0 120 80\""), "{markup}");
        assert!(markup.contains("preserveAspectRatio=\"xMidYMid meet\""));
        assert!(markup.contains("markerHeight=\"8\""));
        assert!(markup.contains("xlink:href=\"#dot\""));
        assert!(markup.contains("<path"));
        assert!(markup.contains("max-width: 90px"));
        assert_eq!(
            embedded_svg_markup(&[HtmlDocumentNode::Text("plain".to_string())]),
            None
        );
    }

    #[test]
    fn rejects_container_and_attribute_operations_on_text_nodes() {
        let mut document = HtmlDocument::parse("<p id=target>Visible</p>");
        let target = must_some(
            document.get_element_by_id("target"),
            "target element must exist",
        );
        must(document.set_text_content(target, "replacement"));
        let text_node = document.next_node_id - 1;

        let append = document.append_child(text_node, target);
        let attribute = document.attribute(text_node, "class");
        let set_attribute = document.set_attribute(text_node, "class", "note");

        assert!(matches!(append, Err(error) if error.contains("not a container")));
        assert!(matches!(attribute, Err(error) if error.contains("not an element")));
        assert!(matches!(set_attribute, Err(error) if error.contains("not an element")));
    }

    fn embedded_svg_markup(nodes: &[HtmlDocumentNode]) -> Option<&str> {
        nodes.iter().find_map(|node| match node {
            HtmlDocumentNode::Element {
                tag,
                attributes,
                children,
                ..
            } => (tag == "svg")
                .then(|| {
                    attributes
                        .iter()
                        .find(|(name, _)| name == EMBEDDED_SVG_MARKUP_ATTRIBUTE)
                        .map(|(_, value)| value.as_str())
                })
                .flatten()
                .or_else(|| embedded_svg_markup(children)),
            HtmlDocumentNode::Text(_) => None,
        })
    }

    #[test]
    fn rejects_external_scripts_and_missing_selectors() {
        let mut document =
            HtmlDocument::parse("<script src=\"app.js\"></script><p class=visible>Visible</p>");

        assert!(matches!(
            document.inline_scripts(),
            Err(error) if error.contains("external script is not supported: app.js")
        ));
        assert_eq!(document.query_selector(".missing"), None);
        assert!(document.query_selector_all("main + p").is_empty());
        assert_eq!(document.get_element_by_id("missing"), None);
    }

    #[test]
    fn closest_selector_returns_no_match_for_invalid_selector_syntax() {
        let mut document = HtmlDocument::parse("<main id=target>Visible</main>");
        let target = must_some(document.get_element_by_id("target"), "target must exist");

        assert_eq!(document.closest_selector(target, "main +"), Ok(None));
    }

    #[test]
    fn rejects_operations_on_missing_nodes() {
        let mut document = HtmlDocument::parse("<p id=target>Visible</p>");
        let target = must_some(document.get_element_by_id("target"), "target must exist");

        assert_missing_node(document.append_child(999, target));
        assert_missing_node(document.append_child(target, 999));
        assert_missing_node(document.remove(999));
        assert_missing_node(document.text_content(999));
        assert_missing_node(document.set_text_content(999, "replacement"));
        assert_missing_node(document.set_attribute(999, "class", "note"));
    }

    #[test]
    #[should_panic(expected = "unexpected test error: boom")]
    fn must_reports_unexpected_test_errors() {
        let _: () = must(Err("boom".to_string()));
    }

    #[test]
    #[should_panic(expected = "target missing")]
    fn must_some_reports_missing_test_values() {
        let _: () = must_some(None, "target missing");
    }

    #[test]
    fn must_some_error_branch_covers_node_ids() {
        assert!(
            std::panic::catch_unwind(|| {
                let _: u64 = must_some(None, "target missing");
            })
            .is_err()
        );
    }

    fn must<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => fail(format!("unexpected test error: {error}")),
        }
    }

    fn must_some<T>(value: Option<T>, message: &str) -> T {
        match value {
            Some(value) => value,
            None => fail(message.to_string()),
        }
    }

    fn fail(message: String) -> ! {
        std::panic::resume_unwind(Box::new(message))
    }

    fn assert_missing_node<T>(result: Result<T, String>) {
        assert_eq!(
            result.err(),
            Some("HTML node 999 does not exist".to_string())
        );
    }
}
