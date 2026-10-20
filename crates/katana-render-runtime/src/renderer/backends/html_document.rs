use std::collections::HashMap;

use markup5ever_rcdom::Handle;
use sha2::{Digest, Sha256};

const IMAGE_SOURCE_DIGEST_SIZE: usize = 32;

#[path = "html_document_attributes.rs"]
mod attributes;
#[path = "html_document_mutation.rs"]
mod mutation;
#[path = "html_document_projection.rs"]
mod projection;
use projection::attributes;
#[path = "html_document_form.rs"]
mod form;
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
    pub(super) body_onload_source: Option<String>,
    pub(super) source_order: Vec<Handle>,
    pub(super) body_onload_source_order_index: Option<usize>,
    pub(super) later_body_onload_tokens: Vec<LaterBodyOnloadToken>,
    nodes: HashMap<u64, Handle>,
    node_ids: HashMap<usize, u64>,
    select_value_empty_overrides: std::collections::HashSet<u64>,
    next_node_id: u64,
    tree_revision: u64,
    /* WHY: 生 URL は大きいため保持せず、作成者属性と独立した固定長キーで実使用量を抑える。 */
    prevalidated_image_events: HashMap<(usize, [u8; IMAGE_SOURCE_DIGEST_SIZE]), &'static str>,
}

pub(super) struct LaterBodyOnloadToken {
    pub(super) script_index: usize,
    pub(super) source_order_index: usize,
    pub(super) source: String,
}

#[derive(Debug, PartialEq)]
pub(super) enum HtmlDocumentScript {
    Source(String),
    LaterBodyOnload(String),
}

impl HtmlDocumentScript {
    pub(super) fn source(&self) -> &str {
        match self {
            Self::Source(source) | Self::LaterBodyOnload(source) => source,
        }
    }

    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Source(_) => "inline-script",
            Self::LaterBodyOnload(_) => "krr-html-later-body-onload",
        }
    }
}

impl LaterBodyOnloadToken {
    pub(super) fn record_source_order(
        tokens: &mut std::iter::Peekable<std::slice::IterMut<'_, Self>>,
        event_index: usize,
        source_order_index: usize,
    ) {
        while let Some(token) = tokens.next_if(|token| token.source_order_index == event_index) {
            token.source_order_index = source_order_index;
        }
    }
}

impl HtmlDocument {
    pub(super) fn clear_prevalidated_image_events(&mut self) {
        self.prevalidated_image_events.clear();
    }

    pub(super) fn cache_prevalidated_image_event(&mut self, source: String, event: &'static str) {
        self.prevalidated_image_events
            .insert(image_source_identity(&source), event);
    }

    pub(super) fn prevalidated_image_event(&self, source: &str) -> Option<&'static str> {
        self.prevalidated_image_events
            .get(&image_source_identity(source))
            .copied()
    }

    pub(crate) fn tree_revision(&self) -> String {
        self.tree_revision.to_string()
    }

    pub(super) fn advance_tree_revision(&mut self) {
        self.tree_revision = self.tree_revision.wrapping_add(1);
    }
}

fn image_source_identity(source: &str) -> (usize, [u8; IMAGE_SOURCE_DIGEST_SIZE]) {
    (source.len(), Sha256::digest(source.as_bytes()).into())
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
    use super::{EMBEDDED_SVG_MARKUP_ATTRIBUTE, HtmlDocument, HtmlDocumentNode};
    use std::collections::HashMap;

    #[test]
    fn document_root_is_never_projected_as_an_interactive_node() -> Result<(), String> {
        let document = HtmlDocument::parse("<p>Visible</p>");
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let [HtmlDocumentNode::Element { node_id, tag, .. }] = nodes.as_slice() else {
            return Err("the HTML element must remain the rendered root".to_string());
        };
        assert_eq!(tag, "html");
        assert_eq!(document.tag_name(*node_id).as_deref(), Ok("HTML"));
        assert_ne!(
            document
                .node_ids
                .get(&(std::rc::Rc::as_ptr(&document.document) as usize)),
            Some(node_id)
        );
        Ok(())
    }

    #[test]
    fn embedded_svg_projection_preserves_vector_markup_and_attribute_case() -> Result<(), String> {
        let document = HtmlDocument::parse(
            r##"<style>svg { max-width: 90px; }</style><svg viewBox="0 0 120 80" preserveAspectRatio="xMidYMid meet" xmlns:xlink="http://www.w3.org/1999/xlink"><!-- marker --><defs><marker id="dot" markerHeight="8"><path d="M0 0L8 4L0 8Z"/></marker></defs><use xlink:href="#dot"/><circle cx="40" cy="40" r="20" fill="#22c55e"/></svg>"##,
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let markup = embedded_svg_markup(&nodes)
            .ok_or_else(|| "embedded SVG markup must exist".to_string())?;

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
        Ok(())
    }

    #[test]
    fn natural_svg_projection_preserves_xml_namespaces_and_xlink_attributes() -> Result<(), String>
    {
        let document = HtmlDocument::parse(
            r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><title>diagram icon</title><image xlink:href="asset.svg"/></svg>"##,
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let markup = embedded_svg_markup(&nodes)
            .ok_or_else(|| "embedded SVG markup must exist".to_string())?;
        assert!(markup.contains("<title>diagram icon</title>"), "{markup}");
        let parsed_svg = xmltree::Element::parse(markup.as_bytes());
        assert!(
            parsed_svg.is_ok(),
            "embedded SVG should parse; markup={markup}"
        );
        let parsed_svg = parsed_svg.map_err(|error| format!("unexpected test error: {error}"))?;
        assert_svg_namespace_mappings(&parsed_svg)?;
        assert_svg_image_attributes(&parsed_svg, markup)?;
        Ok(())
    }

    fn assert_svg_namespace_mappings(svg: &xmltree::Element) -> Result<(), String> {
        assert_eq!(svg.namespace.as_deref(), Some("http://www.w3.org/2000/svg"));
        let namespaces = svg
            .namespaces
            .as_ref()
            .ok_or_else(|| "SVG namespace mappings must exist".to_string())?;
        assert_eq!(namespaces.get(""), Some("http://www.w3.org/2000/svg"));
        assert_eq!(
            namespaces.get("xlink"),
            Some("http://www.w3.org/1999/xlink")
        );
        Ok(())
    }

    fn assert_svg_image_attributes(svg: &xmltree::Element, markup: &str) -> Result<(), String> {
        let image = svg.children.iter().find_map(|child| match child {
            xmltree::XMLNode::Element(element) if element.name == "image" => Some(element),
            _ => None,
        });
        let image = image.ok_or_else(|| "image element must remain in SVG markup".to_string())?;
        assert_eq!(
            image.namespace.as_deref(),
            Some("http://www.w3.org/2000/svg")
        );
        assert_eq!(
            image.attributes.get("href").map(String::as_str),
            Some("asset.svg")
        );
        assert!(markup.contains("xlink:href=\"asset.svg\""), "{markup}");
        Ok(())
    }

    #[test]
    fn rejects_container_and_attribute_operations_on_text_nodes() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<p id=target>Visible</p>");
        let target = document
            .get_element_by_id("target")
            .ok_or_else(|| "target element must exist".to_string())?;
        document
            .set_text_content(target, "replacement")
            .map_err(|error| format!("unexpected test error: {error}"))?;
        let text_node = document.next_node_id - 1;

        let append = document.append_child(text_node, target);
        let attribute = document.attribute(text_node, "class");
        let set_attribute = document.set_attribute(text_node, "class", "note");

        assert!(matches!(append, Err(error) if error.contains("not a container")));
        assert!(matches!(attribute, Err(error) if error.contains("not an element")));
        assert!(matches!(set_attribute, Err(error) if error.contains("not an element")));
        Ok(())
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
    fn closest_selector_returns_no_match_for_invalid_selector_syntax() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<main id=target>Visible</main>");
        let target = document
            .get_element_by_id("target")
            .ok_or_else(|| "target must exist".to_string())?;

        assert_eq!(document.closest_selector(target, "main +"), Ok(None));
        Ok(())
    }

    #[test]
    fn rejects_operations_on_missing_nodes() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<p id=target>Visible</p>");
        let target = document
            .get_element_by_id("target")
            .ok_or_else(|| "target must exist".to_string())?;

        assert_missing_node(document.append_child(999, target));
        assert_missing_node(document.append_child(target, 999));
        assert_missing_node(document.remove(999));
        assert_missing_node(document.text_content(999));
        assert_missing_node(document.set_text_content(999, "replacement"));
        assert_missing_node(document.set_attribute(999, "class", "note"));
        Ok(())
    }

    fn assert_missing_node<T>(result: Result<T, String>) {
        assert_eq!(
            result.err(),
            Some("HTML node 999 does not exist".to_string())
        );
    }
}

#[cfg(test)]
#[path = "html_document_form_test_support.rs"]
mod form_test_support;

#[cfg(test)]
#[path = "html_document_form_tests.rs"]
mod form_tests;
