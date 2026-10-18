use super::html_css_rule::{CssRule, parse_rules};
use super::html_css_selector::{CssAncestor, CssPseudoElement};
use super::html_css_sources::{INTERACTIVE_USER_AGENT_STYLES, inline_styles, interactive_styles};
use markup5ever_rcdom::Handle;
use std::collections::HashMap;

#[path = "html_css_apply.rs"]
mod apply;
#[path = "html_css_generated_content.rs"]
mod generated_content;
#[path = "html_css_pseudo.rs"]
mod pseudo;
#[path = "html_css_resolution.rs"]
mod resolution;

pub(super) use generated_content::{CssGeneratedContent, CssPseudoStyle};

pub(super) type HtmlAttributes = Vec<(String, String)>;

pub(super) struct CssPseudoRequest<'a> {
    pub(super) tag: &'a str,
    pub(super) attributes: &'a HtmlAttributes,
    pub(super) ancestors: &'a [CssAncestor],
    pub(super) inheritance_ancestors: &'a [CssAncestor],
    pub(super) sibling_index: usize,
    pub(super) sibling_type_index: usize,
    pub(super) sibling_type_count: usize,
    pub(super) hovered: bool,
    pub(super) focused: bool,
    pub(super) focus_visible: bool,
    pub(super) pseudo_element: CssPseudoElement,
}

const DEFAULT_CSS_VIEWPORT_WIDTH: f32 = 1024.0;

#[derive(Debug)]
pub(super) struct StaticCss {
    rules: Vec<CssRule>,
    mode: CssResolutionMode,
    viewport_width: f32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum CssResolutionMode {
    #[default]
    StaticSnapshot,
    InteractiveRuntime,
}

impl StaticCss {
    pub(super) fn from_document(document: &Handle) -> Self {
        Self::with_mode(document, CssResolutionMode::StaticSnapshot)
    }

    pub(super) fn for_interactive_document_with_styles_at_width(
        document: &Handle,
        external_stylesheets: &HashMap<String, String>,
        viewport_width: f32,
    ) -> Self {
        let source = interactive_styles(document, external_stylesheets);
        let mut rules = parse_rules(&source);
        let user_agent_count = parse_rules(INTERACTIVE_USER_AGENT_STYLES).len();
        for rule in rules.iter_mut().take(user_agent_count) {
            rule.user_agent = true;
        }
        Self {
            rules,
            mode: CssResolutionMode::InteractiveRuntime,
            viewport_width,
        }
    }

    fn with_mode(document: &Handle, mode: CssResolutionMode) -> Self {
        let source = inline_styles(document);
        Self {
            rules: parse_rules(&source),
            mode,
            viewport_width: DEFAULT_CSS_VIEWPORT_WIDTH,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::html_css_cascade::resolve_css_variables;
    use super::super::html_document::{HtmlDocument, HtmlDocumentNode};
    use super::{CssPseudoElement, CssPseudoRequest, HtmlAttributes, StaticCss};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn author_control_styles_override_browser_defaults() -> Result<(), String> {
        let document = HtmlDocument::parse(
            "<style>* { font-size: 18px; } button { border: 0; background: #123456; text-align: left; }</style><button>Action</button>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let style = find_style_attribute(&nodes, "button").ok_or("missing button styles")?;
        assert!(style.contains("font-size: 18px"), "{style}");
        assert!(style.contains("#123456"), "{style}");
        assert!(style.contains("text-align: left"), "{style}");
        assert!(!style.contains("#efefef"), "{style}");
        Ok(())
    }

    #[test]
    fn author_universal_reset_overrides_user_agent_body_margin() -> Result<(), String> {
        let document = HtmlDocument::parse(
            "<style>*, *::before, *::after { margin: 0; padding: 0; }</style><p>Reset</p>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let style = find_style_attribute(&nodes, "body")
            .ok_or_else(|| "missing body styles".to_string())?;
        assert!(style.contains("margin-left: 0"), "{style}");
        assert!(!style.contains("8px"), "{style}");
        Ok(())
    }

    #[test]
    fn longhand_padding_is_resolved_before_shorthand() {
        let document = HtmlDocument::parse(
            "<style>.card { padding-left: 20px; } div { padding: 0; }</style>prefix<div class='card'>A</div>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let style = find_style_attribute(&nodes, "div");

        assert_eq!(
            style.as_deref(),
            Some("padding-left: 20px; padding-top: 0; padding-right: 0; padding-bottom: 0")
        );
    }

    #[test]
    fn later_longhand_wins_after_an_intervening_shorthand() {
        let document = HtmlDocument::parse(
            "<style>.card { padding-left: 20px; padding: 0; padding-left: 10px; }</style><div class='card'>A</div>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let style = find_style_attribute(&nodes, "div");

        assert_eq!(
            style.as_deref(),
            Some("padding-left: 10px; padding-top: 0; padding-right: 0; padding-bottom: 0")
        );
    }

    #[test]
    fn comma_separated_tag_reset_participates_in_the_author_cascade() {
        let document = HtmlDocument::parse(
            "<style>h1, h2, p { margin: 0; } .heading { margin-bottom: 20px; }</style><h1 class=heading>Title</h1>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let style = find_style_attribute(&nodes, "h1");

        assert!(
            style.as_ref().is_some_and(|style| {
                style.contains("margin-top: 0")
                    && style.contains("margin-right: 0")
                    && style.contains("margin-bottom: 20px")
                    && style.contains("margin-left: 0")
            }),
            "tag reset was missing from computed style: {style:?}"
        );
    }

    #[test]
    fn important_inline_and_custom_properties_follow_cascade_precedence() {
        let document = HtmlDocument::parse(
            "<style>:root { --accent: #123456; } .card { color: red !important; } #card { color: blue; }</style><div id=card class=card style='color: var(--accent)'>A</div>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let style = find_style_attribute(&nodes, "div");
        assert!(
            style.as_ref().is_some_and(|style| {
                style.contains("color: red") && style.contains("--accent: #123456")
            }),
            "computed style did not preserve cascade precedence: {style:?}"
        );
    }

    #[test]
    fn nth_child_cascade_uses_element_sibling_order_not_text_nodes() {
        let document = HtmlDocument::parse(
            "<style>.layer { color: red; }.layer:nth-child(3) { color: blue; }</style><main>\n<div id=first class=layer>A</div>\n<span>B</span>\n<div id=third class=layer>C</div>\n</main>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());

        assert_eq!(
            find_style_for_id(&nodes, "first").as_deref(),
            Some("color: red")
        );
        assert_eq!(
            find_style_for_id(&nodes, "third").as_deref(),
            Some("color: blue")
        );
    }

    #[test]
    fn css_variables_resolve_nested_values_fallbacks_and_cycles() {
        let custom = HashMap::from([
            ("--a".to_string(), "var(--b)".to_string()),
            ("--b".to_string(), "12px".to_string()),
            ("--cycle".to_string(), "var(--cycle)".to_string()),
        ]);

        assert_eq!(
            resolve_css_variables("calc(var(--a) + 2px)", &custom, &mut HashSet::new()),
            Some("calc(12px + 2px)".to_string())
        );
        assert_eq!(
            resolve_css_variables("var(--missing, red)", &custom, &mut HashSet::new()),
            Some("red".to_string())
        );
        assert_eq!(
            resolve_css_variables("var(--cycle)", &custom, &mut HashSet::new()),
            None
        );
    }

    #[test]
    fn pseudo_style_is_unavailable_in_static_mode() {
        let document =
            HtmlDocument::parse("<style>div:before { content: 'x' }</style><div>Test</div>");
        let css = StaticCss::from_document(&document.document);
        let style = css.pseudo_style_at_state(CssPseudoRequest {
            tag: "div",
            attributes: &HtmlAttributes::new(),
            ancestors: &[],
            inheritance_ancestors: &[],
            sibling_index: 1,
            sibling_type_index: 1,
            sibling_type_count: 1,
            hovered: false,
            focused: false,
            focus_visible: false,
            pseudo_element: CssPseudoElement::Before,
        });

        assert!(style.is_none());
    }

    fn find_style_attribute(nodes: &[HtmlDocumentNode], target_tag: &str) -> Option<String> {
        nodes.iter().find_map(|node| match node {
            HtmlDocumentNode::Element {
                tag,
                attributes,
                children,
                ..
            } => {
                if tag == target_tag {
                    attributes
                        .iter()
                        .find(|(name, _)| name == "style")
                        .map(|(_, value)| value.clone())
                } else {
                    find_style_attribute(children, target_tag)
                }
            }
            HtmlDocumentNode::Text(_) => None,
        })
    }

    fn find_style_for_id(nodes: &[HtmlDocumentNode], target_id: &str) -> Option<String> {
        nodes.iter().find_map(|node| match node {
            HtmlDocumentNode::Element {
                attributes,
                children,
                ..
            } => {
                let matches = attributes
                    .iter()
                    .any(|(name, value)| name == "id" && value == target_id);
                if matches {
                    attributes
                        .iter()
                        .find(|(name, _)| name == "style")
                        .map(|(_, value)| value.clone())
                } else {
                    find_style_for_id(children, target_id)
                }
            }
            HtmlDocumentNode::Text(_) => None,
        })
    }
}
