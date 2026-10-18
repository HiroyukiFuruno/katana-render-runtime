use super::html_dom_helpers::text_content;
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;

pub(super) const INTERACTIVE_USER_AGENT_STYLES: &str = r#"body { margin: 8px; }
a, abbr, b, bdi, bdo, cite, code, data, del, dfn, em, i, ins, kbd, label, mark, q, s, samp, small, span, strong, sub, sup, time, u, var { display: inline; }
a { color: #0000ee; text-decoration: underline; }
button, input, textarea, select { font-family: Arial; font-size: 13.333333px; font-weight: 400; color: #000000; line-height: normal; }
button { background: #efefef; border: 2px solid #767676; border-radius: 2px; padding: 1px 6px; text-align: center; }
input, textarea { background: #ffffff; border: 2px solid #767676; border-radius: 2px; padding: 1px 2px; }
b, strong { font-weight: 700; }
em, i { font-style: italic; }
ul, ol { margin-top: 1em; margin-bottom: 1em; padding-left: 40px; }
"#;

pub(super) fn inline_styles(document: &Handle) -> String {
    let mut source = String::new();
    collect_inline_styles(document, &mut source);
    source
}

pub(super) fn interactive_styles(
    document: &Handle,
    external_stylesheets: &HashMap<String, String>,
) -> String {
    let mut source = INTERACTIVE_USER_AGENT_STYLES.to_string();
    collect_interactive_styles(document, external_stylesheets, &mut source);
    source
}

fn collect_inline_styles(node: &Handle, source: &mut String) {
    if element_name(node).is_some_and(|name| name == "style") {
        append_text(node, source);
        return;
    }
    for child in node.children.borrow().iter() {
        collect_inline_styles(child, source);
    }
}

fn collect_interactive_styles(
    node: &Handle,
    external_stylesheets: &HashMap<String, String>,
    source: &mut String,
) {
    if let Some(name) = element_name(node) {
        if name == "style" {
            append_text(node, source);
            return;
        }
        if name == "link"
            && let Some(stylesheet) = stylesheet_reference(node)
                .and_then(|reference| external_stylesheets.get(&reference))
        {
            source.push_str(stylesheet);
            source.push('\n');
            return;
        }
    }
    for child in node.children.borrow().iter() {
        collect_interactive_styles(child, external_stylesheets, source);
    }
}

fn append_text(node: &Handle, source: &mut String) {
    source.push_str(&text_content(node));
    source.push('\n');
}

fn stylesheet_reference(node: &Handle) -> Option<String> {
    let NodeData::Element { attrs, .. } = &node.data else {
        return None;
    };
    let attributes = attrs.borrow();
    let rel = attributes
        .iter()
        .find(|attribute| attribute.name.local.as_str().eq_ignore_ascii_case("rel"))?
        .value
        .to_string();
    rel.split_ascii_whitespace()
        .any(|value| value.eq_ignore_ascii_case("stylesheet"))
        .then(|| {
            attributes
                .iter()
                .find(|attribute| attribute.name.local.as_str().eq_ignore_ascii_case("href"))
                .map(|attribute| attribute.value.to_string())
        })
        .flatten()
}

fn element_name(node: &Handle) -> Option<String> {
    match &node.data {
        NodeData::Element { name, .. } => Some(name.local.to_string().to_ascii_lowercase()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        INTERACTIVE_USER_AGENT_STYLES, inline_styles, interactive_styles, stylesheet_reference,
    };
    use crate::renderer::backends::html_document::HtmlDocument;
    use std::collections::HashMap;

    #[test]
    fn interactive_styles_preserve_document_order_and_ignore_non_elements() {
        let document = HtmlDocument::parse(
            "<style>#target { color: red; }</style><link rel=stylesheet href=theme.css><p id=target>Visible</p>",
        );
        let stylesheets = HashMap::from([(
            "theme.css".to_string(),
            "#target { color: blue; }".to_string(),
        )]);
        let source = interactive_styles(&document.document, &stylesheets);

        assert!(
            source
                .find("color: red")
                .zip(source.find("color: blue"))
                .is_some_and(|(inline, external)| inline < external)
        );
        assert_eq!(stylesheet_reference(&document.document), None);
        assert!(inline_styles(&document.document).contains("color: red"));

        let non_stylesheet = HtmlDocument::parse("<link href=theme.css>");
        assert_eq!(
            interactive_styles(&non_stylesheet.document, &stylesheets),
            INTERACTIVE_USER_AGENT_STYLES
        );
    }
}
