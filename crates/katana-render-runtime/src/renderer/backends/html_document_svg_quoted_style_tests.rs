use crate::markdown::svg_rasterize::SvgRasterizeOps;
use crate::renderer::backends::html_document::{
    EMBEDDED_SVG_MARKUP_ATTRIBUTE, HtmlDocument, HtmlDocumentNode,
};
use std::collections::HashMap;
use xmltree::Element;

const QUOTED_ONLY_CSS: &str = r##"#fixture [id$="-tip"] {fill:transparent!important;stroke:#aaa!important;stroke-width:2}.box {filter:drop-shadow(1px 2px 2px rgba(185,185,185,1))}"##;
const ENTITY_CSS: &str = r##".shape[data-label="a&b"] {fill:red}"##;
const ENTITY_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="80"><style>.shape[data-label="a&b"] {fill:red}</style><text data-label="a&quot;b">A &amp; B &lt; C &gt; D "quoted" 'literal'</text></svg>"##;

#[test]
fn svg_projection_preserves_quoted_css_as_literal_xml_text() {
    with_projected_markup(&quoted_svg(), |markup| {
        with_style_body(markup, |raw| {
            assert_eq!(raw, QUOTED_ONLY_CSS);
            with_decoded_style(markup, |decoded| assert_eq!(raw, decoded));
        });
    });
}

#[test]
fn svg_projection_preserves_xml_text_and_attribute_escaping() {
    with_projected_markup(ENTITY_SVG, |markup| {
        assert!(markup.contains(r##"data-label="a&quot;b""##));
        assert!(markup.contains(r##"A &amp; B &lt; C &gt; D "quoted" 'literal'"##));
        with_style_body(markup, |raw| {
            assert!(raw.contains(r##"[data-label="a&amp;b"]"##));
        });
        with_decoded_style(markup, |decoded| assert_eq!(decoded, ENTITY_CSS));
    });
}

#[test]
fn svg_projection_quoted_css_reaches_marker_and_shadow_normalization() {
    with_projected_markup(&quoted_svg(), |markup| {
        let normalized = SvgRasterizeOps::preprocess_for_rasterizer(markup);
        assert!(normalized.contains(r##"#fixture [id="head-tip"]"##));
        assert!(normalized.contains("#B9B9B9FF"));
        assert!(!normalized.contains("rgba(185,185,185,1)"));
    });
}

fn quoted_svg() -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" id="fixture" width="100" height="80"><style>{QUOTED_ONLY_CSS}</style><defs><marker id="head-tip"><path d="M0 0L10 5L0 10Z"/></marker></defs><rect class="box" width="20" height="20"/></svg>"##
    )
}

fn with_projected_markup(svg: &str, action: impl FnOnce(&str)) {
    let nodes = HtmlDocument::parse(svg).interactive_nodes_with_styles(&HashMap::new());
    let markup = embedded_markup(&nodes);
    assert!(markup.is_some(), "projected SVG markup missing");
    if let Some(markup) = markup {
        action(markup);
    }
}

fn with_decoded_style(markup: &str, action: impl FnOnce(&str)) {
    let document = Element::parse(markup.as_bytes());
    assert!(
        document.is_ok_and(|document| {
            let decoded = document.get_child("style").and_then(Element::get_text);
            assert!(decoded.is_some(), "projected SVG style text missing");
            if let Some(decoded) = decoded {
                action(decoded.as_ref());
            }
            true
        }),
        "projected SVG XML invalid"
    );
}

fn embedded_markup(nodes: &[HtmlDocumentNode]) -> Option<&str> {
    nodes.iter().find_map(|node| {
        let HtmlDocumentNode::Element {
            attributes,
            children,
            ..
        } = node
        else {
            return None;
        };
        attributes
            .iter()
            .find(|(name, _)| name == EMBEDDED_SVG_MARKUP_ATTRIBUTE)
            .map(|(_, value)| value.as_str())
            .or_else(|| embedded_markup(children))
    })
}

fn with_style_body(markup: &str, action: impl FnOnce(&str)) {
    let start = markup.find("<style>").map(|index| index + "<style>".len());
    assert!(start.is_some(), "style open missing");
    if let Some(start) = start {
        let end = markup[start..].find("</style>").map(|index| index + start);
        assert!(end.is_some(), "style close missing");
        if let Some(end) = end {
            action(&markup[start..end]);
        }
    }
}
