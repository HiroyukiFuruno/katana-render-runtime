use super::document_scripts::load_scripts;
use super::{HtmlDocumentResources, HtmlSubresourceLoader};
use crate::renderer::backends::html_document::HtmlDocument;
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;

pub(super) fn load_document_resources(
    loader: &HtmlSubresourceLoader,
    document: &mut HtmlDocument,
) -> HtmlDocumentResources {
    super::iframe::inline_iframes(loader, document);
    let stylesheets = load_stylesheets(loader, &document.document);
    let (scripts, body_onload_script_index) = load_scripts(loader, document);
    inline_images(loader, &document.document);
    HtmlDocumentResources {
        stylesheets,
        scripts,
        body_onload_script_index,
    }
}

fn load_stylesheets(loader: &HtmlSubresourceLoader, document: &Handle) -> HashMap<String, String> {
    let mut references = Vec::new();
    collect_stylesheet_references(document, &mut references);
    references
        .into_iter()
        .filter_map(|reference| load_text(loader, "stylesheet", reference))
        .collect()
}

fn collect_stylesheet_references(node: &Handle, references: &mut Vec<String>) {
    if is_stylesheet(node)
        && let Some(href) = attribute(node, "href")
    {
        references.push(href);
    }
    for child in node.children.borrow().iter() {
        collect_stylesheet_references(child, references);
    }
}

fn inline_images(loader: &HtmlSubresourceLoader, node: &Handle) {
    if is_tag(node, "img")
        && let Some(source) = attribute(node, "src")
    {
        match loader.load_image_data_url(&source) {
            Ok(data_url) => set_attribute(node, "src", &data_url),
            Err(error) => {
                log_subresource_failure(loader, "image", &source, &error);
            }
        }
    }
    let children = node.children.borrow().clone();
    for child in children {
        inline_images(loader, &child);
    }
}

pub(super) fn load_text(
    loader: &HtmlSubresourceLoader,
    resource_kind: &'static str,
    reference: String,
) -> Option<(String, String)> {
    match loader.load_text(&reference) {
        Ok(source) => Some((reference, source)),
        Err(error) => {
            log_subresource_failure(loader, resource_kind, &reference, &error);
            None
        }
    }
}

fn log_subresource_failure(
    loader: &HtmlSubresourceLoader,
    resource_kind: &'static str,
    reference: &str,
    error: &str,
) {
    let document_origin = loader.document_origin();
    tracing::warn!(
        layer = "KRR runtime",
        operation = "load_subresource",
        document = document_origin,
        resource_kind,
        resource = reference,
        error,
        "HTML subresource load failed; rendering continues"
    );
}

fn is_stylesheet(node: &Handle) -> bool {
    is_tag(node, "link")
        && attribute(node, "rel").is_some_and(|rel| {
            rel.split_ascii_whitespace()
                .any(|value| value.eq_ignore_ascii_case("stylesheet"))
        })
}

pub(super) fn is_tag(node: &Handle, expected: &str) -> bool {
    matches!(&node.data, NodeData::Element { name, .. } if name.local.as_str().eq_ignore_ascii_case(expected))
}

pub(super) fn attribute(node: &Handle, expected: &str) -> Option<String> {
    let NodeData::Element { attrs, .. } = &node.data else {
        return None;
    };
    attrs
        .borrow()
        .iter()
        .find(|attribute| attribute.name.local.as_str().eq_ignore_ascii_case(expected))
        .map(|attribute| attribute.value.to_string())
}

fn set_attribute(node: &Handle, expected: &str, value: &str) {
    let NodeData::Element { attrs, .. } = &node.data else {
        return;
    };
    if let Some(attribute) = attrs
        .borrow_mut()
        .iter_mut()
        .find(|attribute| attribute.name.local.as_str().eq_ignore_ascii_case(expected))
    {
        attribute.value = value.into();
    }
}

pub(super) fn text_content(node: &Handle) -> String {
    match &node.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        _ => node.children.borrow().iter().map(text_content).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::{attribute, load_document_resources, set_attribute};
    use crate::renderer::backends::html_browser::HtmlBrowserSource;
    use crate::renderer::backends::html_document::HtmlDocument;
    use crate::renderer::backends::html_runtime::StaticHtmlRuntime;
    use crate::renderer::backends::html_subresources::HtmlSubresourceLoader;

    fn must_result<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        assert!(result.is_ok());
        let mut values = result.into_iter().collect::<Vec<_>>();
        values.remove(0)
    }

    #[test]
    fn loaded_script_index_tracks_body_onload_parser_position() {
        let source = must_result(HtmlBrowserSource::new(
            r#"<head><script>head</script></head><body onload="body"><script>body</script></body>"#,
            "https://example.test/index.html",
        ));
        let loader = HtmlSubresourceLoader::new(&source);
        let mut document = HtmlDocument::parse(&source.raw_html);

        let resources = load_document_resources(&loader, &mut document);

        assert_eq!(resources.scripts, ["head", "body"]);
        assert_eq!(resources.body_onload_script_index, Some(1));
    }

    #[test]
    fn later_duplicate_body_onload_token_overwrites_prior_script_handler() {
        let source = must_result(HtmlBrowserSource::new(
            r#"<body><script>window.onload = () => { document.getElementById('status').textContent = 'first'; };</script><body onload="document.getElementById('status').textContent = 'second'"><p id=status></p>"#,
            "https://example.test/index.html",
        ));

        let snapshot =
            must_result(must_result(StaticHtmlRuntime.start_interactive(&source)).snapshot());
        assert!(
            snapshot.contains(r#"<p id="status">second</p>"#),
            "{snapshot}"
        );
    }

    #[test]
    fn failed_external_script_before_body_does_not_override_later_inline_onload() {
        let source = must_result(HtmlBrowserSource::new(
            r#"<script src="data:text/javascript,%FF"></script><body onload="document.getElementById('status').textContent = 'second'"><script>window.onload = () => { document.getElementById('status').textContent = 'first'; };</script><p id=status></p>"#,
            "https://example.test/index.html",
        ));

        let snapshot =
            must_result(must_result(StaticHtmlRuntime.start_interactive(&source)).snapshot());
        assert!(
            snapshot.contains(r#"<p id="status">first</p>"#),
            "{snapshot}"
        );
    }

    #[test]
    fn non_element_attribute_helpers_do_not_create_or_read_values() {
        let document = HtmlDocument::parse("Visible");

        assert_eq!(attribute(&document.document, "src"), None);
        set_attribute(&document.document, "src", "data:image/png;base64,AA==");
        assert_eq!(attribute(&document.document, "src"), None);
    }

    #[test]
    fn element_attribute_helper_does_not_create_missing_attributes() {
        let mut document = HtmlDocument::parse("<img id=image>");
        let image = document.get_element_by_id("image");

        assert!(image.is_some());
        image.iter().for_each(|image| {
            let node = document.node(*image);
            assert!(node.is_ok());
            node.iter().for_each(|node| {
                set_attribute(node, "src", "data:image/png;base64,AA==");
                assert_eq!(attribute(node, "src"), None);
            });
        });
    }
}
