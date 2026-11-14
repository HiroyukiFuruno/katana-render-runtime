use super::super::super::html_css::{
    CssGeneratedContent, CssPseudoRequest, CssPseudoStyle, HtmlAttributes,
};
use super::super::super::html_css_selector::{CssAncestor, CssPseudoElement};
use super::super::{HtmlDocument, HtmlDocumentNode};
use super::InteractiveElementProjection;
use base64::Engine as _;
use percent_encoding::percent_decode_str;
use std::collections::{HashMap, HashSet};

const GENERATED_NODE_FLAG: u64 = 1 << 63;
const GENERATED_NODE_SHIFT: u32 = 3;
const GENERATED_HOST_MASK: u64 = (1 << (63 - GENERATED_NODE_SHIFT)) - 1;
const BEFORE_NODE_SLOT: u64 = 1;
const AFTER_NODE_SLOT: u64 = 3;
const GENERATED_CONTENT_NODE_OFFSET: u64 = 1;
const GENERATED_SVG_VIEWPORT_WIDTH: f32 = 1024.0;

struct ProjectedSvg {
    tag: String,
    attributes: HtmlAttributes,
    children: Vec<HtmlDocumentNode>,
}

impl InteractiveElementProjection<'_> {
    pub(super) fn pseudo_node(
        &self,
        host_node_id: u64,
        pseudo_element: CssPseudoElement,
        hovered: bool,
        focused: bool,
        focus_visible: bool,
        inheritance_ancestors: &[CssAncestor],
    ) -> Option<HtmlDocumentNode> {
        let (attributes, content) = self.resolve_pseudo_style(
            pseudo_element,
            hovered,
            focused,
            focus_visible,
            inheritance_ancestors,
        )?;
        let slot = self.pseudo_slot(pseudo_element);
        Some(projected_pseudo_node(
            host_node_id,
            slot,
            pseudo_element,
            attributes,
            content,
        ))
    }

    fn resolve_pseudo_style(
        &self,
        pseudo_element: CssPseudoElement,
        hovered: bool,
        focused: bool,
        focus_visible: bool,
        inheritance_ancestors: &[CssAncestor],
    ) -> Option<(HtmlAttributes, CssGeneratedContent)> {
        let CssPseudoStyle {
            attributes,
            content,
        } = self.css.pseudo_style_at_state(CssPseudoRequest {
            tag: &self.tag,
            attributes: &self.source_attributes,
            ancestors: self.ancestors,
            inheritance_ancestors,
            sibling_index: self.sibling_index,
            sibling_type_index: self.sibling_type_index,
            sibling_type_count: self.sibling_type_count,
            hovered,
            focused,
            focus_visible,
            pseudo_element,
        })?;
        Some((attributes, content))
    }

    fn pseudo_slot(&self, pseudo_element: CssPseudoElement) -> u64 {
        match pseudo_element {
            CssPseudoElement::Before => BEFORE_NODE_SLOT,
            CssPseudoElement::After => AFTER_NODE_SLOT,
        }
    }
}

fn projected_pseudo_node(
    host_node_id: u64,
    slot: u64,
    pseudo_element: CssPseudoElement,
    mut attributes: HtmlAttributes,
    content: CssGeneratedContent,
) -> HtmlDocumentNode {
    let pseudo_name = match pseudo_element {
        CssPseudoElement::Before => "before",
        CssPseudoElement::After => "after",
    };
    attributes.push(("data-krr-pseudo".to_string(), pseudo_name.to_string()));
    let children = match content {
        CssGeneratedContent::Text(text) => vec![HtmlDocumentNode::Text(text)],
        CssGeneratedContent::Image(source) => vec![generated_image_node(
            host_node_id,
            slot + GENERATED_CONTENT_NODE_OFFSET,
            source,
        )],
    };
    HtmlDocumentNode::Element {
        node_id: generated_node_id(host_node_id, slot),
        tag: "span".to_string(),
        attributes,
        children,
    }
}

fn generated_node_id(host_node_id: u64, slot: u64) -> u64 {
    GENERATED_NODE_FLAG | ((host_node_id & GENERATED_HOST_MASK) << GENERATED_NODE_SHIFT) | slot
}

fn generated_image_node(host_node_id: u64, slot: u64, source: String) -> HtmlDocumentNode {
    decoded_svg_data(&source)
        .and_then(project_data_svg)
        .map(|svg| HtmlDocumentNode::Element {
            node_id: generated_node_id(host_node_id, slot),
            tag: svg.tag,
            attributes: svg.attributes,
            children: svg.children,
        })
        .unwrap_or_else(|| fallback_image_node(host_node_id, slot, source))
}

fn project_data_svg(svg: String) -> Option<ProjectedSvg> {
    let document = HtmlDocument::parse(&svg);
    document
        .interactive_nodes_with_styles_at_width_and_hover(
            &HashMap::new(),
            GENERATED_SVG_VIEWPORT_WIDTH,
            &HashSet::new(),
        )
        .into_iter()
        .find_map(take_embedded_svg)
}

fn fallback_image_node(host_node_id: u64, slot: u64, source: String) -> HtmlDocumentNode {
    HtmlDocumentNode::Element {
        node_id: generated_node_id(host_node_id, slot),
        tag: "img".to_string(),
        attributes: vec![("src".to_string(), source)],
        children: Vec::new(),
    }
}

fn take_embedded_svg(node: HtmlDocumentNode) -> Option<ProjectedSvg> {
    match node {
        HtmlDocumentNode::Element {
            tag,
            attributes,
            children,
            ..
        } if tag == "svg" => Some(ProjectedSvg {
            tag,
            attributes,
            children,
        }),
        HtmlDocumentNode::Element { children, .. } => {
            children.into_iter().find_map(take_embedded_svg)
        }
        HtmlDocumentNode::Text(_) => None,
    }
}

fn decoded_svg_data(source: &str) -> Option<String> {
    let data = source.strip_prefix("data:image/svg+xml")?;
    let (metadata, payload) = data.split_once(',')?;
    let bytes = if metadata
        .split(';')
        .any(|part| part.eq_ignore_ascii_case("base64"))
    {
        base64::engine::general_purpose::STANDARD
            .decode(payload)
            .ok()?
    } else {
        percent_decode_str(payload).collect()
    };
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::super::super::{EMBEDDED_SVG_MARKUP_ATTRIBUTE, HtmlDocument, HtmlDocumentNode};
    use std::collections::HashMap;

    #[test]
    fn generated_pseudo_content_wraps_real_children_in_source_order() -> Result<(), String> {
        let document = HtmlDocument::parse(
            "<style>.target:before{content:'before'}.target:after{content:'after'}</style><div class=target>real</div>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let projected = find_element(&nodes, "div").ok_or("projected div was missing")?;
        let projected = html_node_children(projected).ok_or("projected div is not an element")?;
        let before = find_span_with_data_krr_pseudo(projected, "before")
            .ok_or("generated before pseudo node must exist")?;
        let after = find_span_with_data_krr_pseudo(projected, "after")
            .ok_or("generated after pseudo node must exist")?;
        assert_eq!(text(Some(before)), Some("before"));
        assert_eq!(text(Some(after)), Some("after"));
        assert_eq!(text(projected.first()), Some("before"));
        assert_eq!(text(projected.get(1)), Some("real"));
        assert_eq!(text(projected.get(2)), Some("after"));
        Ok(())
    }

    #[test]
    fn generated_url_content_projects_an_image_without_styling_the_host() {
        let document = HtmlDocument::parse(
            "<style>.target:after{content:url('data:image/svg+xml,<svg/>');width:40px}</style><div class=target></div>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let host = find_element(&nodes, "div");
        assert!(host.is_some_and(host_has_no_style_attribute));
        assert!(host.is_some_and(host_has_generated_svg_after));
    }

    #[test]
    fn generated_svg_content_decodes_percent_and_base64_data_urls() -> Result<(), String> {
        for source in [
            "data:image/svg+xml,%3Csvg%20width%3D%2210%22%3E%3C/svg%3E",
            "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTAiPjwvc3ZnPg==",
        ] {
            assert_generated_svg_data_node(source)?;
        }
        Ok(())
    }

    fn assert_generated_svg_data_node(source: &str) -> Result<(), String> {
        let node = super::generated_image_node(7, 2, source.to_string());
        let node = format!("{node:?}");
        assert!(
            node.contains("tag: \"svg\""),
            "expected projected image to be svg"
        );
        assert!(
            node.contains(EMBEDDED_SVG_MARKUP_ATTRIBUTE),
            "expected embedded svg markup attribute"
        );
        Ok(())
    }

    #[test]
    fn generated_node_id_slots_are_calculated_for_fallback_images() -> Result<(), String> {
        let node = super::generated_image_node(9, 2, "https://example.com/logo.png".to_string());
        let node = format!("{node:?}");
        let generated_node_id = super::generated_node_id(9, 2);
        assert!(
            node.contains("tag: \"img\""),
            "expected fallback node to be img"
        );
        assert!(node.contains("src"), "expected img source attribute");
        assert!(node.contains(&format!("node_id: {generated_node_id}")));
        Ok(())
    }

    #[test]
    fn generated_image_data_without_supported_svg_payload_falls_back_to_img_node() {
        for source in [
            "https://example.com/picture.png",
            "data:image/svg+xml",
            "data:image/svg+xml;base64,%%%",
        ] {
            let document = HtmlDocument::parse(&format!(
                "<style>.target:after{{content:url('{source}')}}</style><div class=target></div>"
            ));
            let nodes = document.interactive_nodes_with_styles(&HashMap::new());
            let image_attributes = find_element(&nodes, "div")
                .and_then(html_node_children)
                .and_then(|children| find_span_with_data_krr_pseudo(children, "after"))
                .and_then(html_node_children)
                .and_then(|children| find_element(children, "img"))
                .and_then(html_node_attributes);
            assert!(
                image_attributes.is_some_and(|attributes| {
                    attributes
                        .iter()
                        .any(|(name, value)| name == "src" && value == source)
                }),
                "expected generated image to preserve source {source}"
            );
        }
    }

    #[test]
    fn take_embedded_svg_text_nodes_are_ignored() {
        assert!(super::take_embedded_svg(HtmlDocumentNode::Text("plain".to_string())).is_none());
        let wrapper = HtmlDocumentNode::Element {
            node_id: 1,
            tag: "div".to_string(),
            attributes: Vec::new(),
            children: vec![HtmlDocumentNode::Text("plain".to_string())],
        };
        assert!(super::take_embedded_svg(wrapper).is_none());
    }

    #[test]
    fn non_svg_data_url_is_not_parsed_as_svg_payload() {
        assert!(super::decoded_svg_data("data:image/jpeg,abc").is_none());
    }

    #[test]
    fn find_element_skips_text_nodes_in_recursive_search() {
        let nodes = vec![
            HtmlDocumentNode::Text("ignore".to_string()),
            HtmlDocumentNode::Element {
                node_id: 1,
                tag: "section".to_string(),
                attributes: Vec::new(),
                children: vec![HtmlDocumentNode::Element {
                    node_id: 2,
                    tag: "target".to_string(),
                    attributes: Vec::new(),
                    children: Vec::new(),
                }],
            },
        ];

        assert!(matches!(
            find_element(&nodes, "target"),
            Some(HtmlDocumentNode::Element { .. })
        ));
    }

    #[test]
    fn text_returns_none_if_first_child_is_not_text() {
        assert_eq!(
            text(Some(&HtmlDocumentNode::Element {
                node_id: 1,
                tag: "div".to_string(),
                attributes: Vec::new(),
                children: vec![HtmlDocumentNode::Element {
                    node_id: 2,
                    tag: "child".to_string(),
                    attributes: Vec::new(),
                    children: Vec::new(),
                }],
            })),
            None
        );
    }

    fn find_element<'a>(nodes: &'a [HtmlDocumentNode], tag: &str) -> Option<&'a HtmlDocumentNode> {
        nodes.iter().find_map(|node| match node {
            HtmlDocumentNode::Element {
                tag: candidate,
                children,
                ..
            } if candidate == tag => Some(node),
            HtmlDocumentNode::Element { children, .. } => find_element(children, tag),
            HtmlDocumentNode::Text(_) => None,
        })
    }

    fn text(node: Option<&HtmlDocumentNode>) -> Option<&str> {
        match node? {
            HtmlDocumentNode::Text(text) => Some(text),
            HtmlDocumentNode::Element { children, .. } => match children.first()? {
                HtmlDocumentNode::Text(text) => Some(text),
                HtmlDocumentNode::Element { .. } => None,
            },
        }
    }

    fn html_node_children(node: &HtmlDocumentNode) -> Option<&Vec<HtmlDocumentNode>> {
        if let HtmlDocumentNode::Element { children, .. } = node {
            Some(children)
        } else {
            None
        }
    }

    fn html_node_attributes(node: &HtmlDocumentNode) -> Option<&Vec<(String, String)>> {
        if let HtmlDocumentNode::Element { attributes, .. } = node {
            Some(attributes)
        } else {
            None
        }
    }

    fn find_span_with_data_krr_pseudo<'a>(
        nodes: &'a [HtmlDocumentNode],
        pseudo: &str,
    ) -> Option<&'a HtmlDocumentNode> {
        nodes.iter().find(|node| {
            let HtmlDocumentNode::Element {
                tag, attributes, ..
            } = node
            else {
                return false;
            };
            if tag != "span" {
                return false;
            }

            attributes
                .iter()
                .any(|(name, value)| name == "data-krr-pseudo" && value == pseudo)
        })
    }

    fn host_has_no_style_attribute(host: &HtmlDocumentNode) -> bool {
        html_node_attributes(host)
            .is_some_and(|attributes| attributes.iter().all(|(name, _)| name != "style"))
    }

    fn host_has_generated_svg_after(host: &HtmlDocumentNode) -> bool {
        html_node_children(host)
            .and_then(|children| find_span_with_data_krr_pseudo(children, "after"))
            .is_some_and(generated_after_has_svg)
    }

    fn generated_after_has_svg(after: &HtmlDocumentNode) -> bool {
        let style_is_applied = html_node_attributes(after).is_some_and(|attributes| {
            attributes
                .iter()
                .any(|(name, value)| name == "style" && value.contains("width: 40px"))
        });
        let svg_is_embedded = html_node_children(after)
            .and_then(|children| find_element(children, "svg"))
            .and_then(html_node_attributes)
            .is_some_and(|attributes| {
                attributes.iter().any(|(name, value)| {
                    name == EMBEDDED_SVG_MARKUP_ATTRIBUTE && value.contains("<svg")
                })
            });
        style_is_applied && svg_is_embedded
    }

    #[test]
    fn helper_node_apis_return_none_for_non_elements() {
        let text = HtmlDocumentNode::Text("plain".to_string());
        let element = HtmlDocumentNode::Element {
            node_id: 1,
            tag: "div".to_string(),
            attributes: Vec::new(),
            children: Vec::new(),
        };

        assert!(html_node_children(&text).is_none());
        assert!(html_node_attributes(&text).is_none());
        assert!(find_span_with_data_krr_pseudo(std::slice::from_ref(&element), "before").is_none());
    }
}
