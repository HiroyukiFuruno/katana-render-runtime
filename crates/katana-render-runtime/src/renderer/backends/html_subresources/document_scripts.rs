use super::HtmlSubresourceLoader;
use super::document::{attribute, is_tag, load_text, text_content};
use crate::renderer::backends::html_document::{HtmlDocument, LaterBodyOnloadToken};
use html5ever::ns;
use markup5ever_rcdom::{Handle, NodeData};

pub(super) fn load_scripts(
    loader: &HtmlSubresourceLoader,
    document: &HtmlDocument,
) -> (Vec<String>, Option<usize>) {
    let mut collector = ScriptCollector::new(loader, document);
    collector.collect(&document.source_order);
    collector.finish()
}

struct ScriptCollector<'a> {
    loader: &'a HtmlSubresourceLoader,
    scripts: Vec<String>,
    body_source_order_index: Option<usize>,
    source_order_index: usize,
    body_onload_script_index: Option<usize>,
    later_body_onload_tokens: &'a [LaterBodyOnloadToken],
    next_later_token: usize,
}

impl<'a> ScriptCollector<'a> {
    fn new(loader: &'a HtmlSubresourceLoader, document: &'a HtmlDocument) -> Self {
        Self {
            loader,
            scripts: Vec::new(),
            body_source_order_index: document.body_onload_source_order_index,
            source_order_index: 0,
            body_onload_script_index: None,
            later_body_onload_tokens: &document.later_body_onload_tokens,
            next_later_token: 0,
        }
    }

    fn collect(&mut self, source_order: &[Handle]) {
        self.record_body_onload_script_index();
        for node in source_order {
            if is_tag(node, "script") {
                self.collect_script(node, false);
            } else if is_html_tag(node, "iframe") {
                self.collect_iframe(node);
            }
            self.source_order_index += 1;
            self.record_body_onload_script_index();
        }
    }

    fn collect_iframe(&mut self, node: &Handle) {
        for child in node.children.borrow().iter() {
            self.collect_inline_frame_node(child);
        }
    }

    fn collect_inline_frame_node(&mut self, node: &Handle) {
        if is_html_tag(node, "script") {
            if let Some(script) = load_script(self.loader, node) {
                self.scripts.push(script);
            }
            return;
        }
        for child in node.children.borrow().iter() {
            self.collect_inline_frame_node(child);
        }
    }

    fn collect_script(&mut self, node: &Handle, _inside_inline_frame: bool) {
        if let Some(script) = load_script(self.loader, node) {
            self.scripts.push(script);
        }
    }

    fn record_body_onload_script_index(&mut self) {
        if self.body_onload_script_index.is_none()
            && self.body_source_order_index == Some(self.source_order_index)
        {
            self.body_onload_script_index = Some(self.scripts.len());
        }
        while let Some(token) = self.later_body_onload_tokens.get(self.next_later_token)
            && token.source_order_index == self.source_order_index
        {
            self.scripts
                .push(HtmlDocument::later_body_onload_install_script(
                    &token.source,
                ));
            self.next_later_token += 1;
        }
    }

    fn finish(self) -> (Vec<String>, Option<usize>) {
        (self.scripts, self.body_onload_script_index)
    }
}

fn is_html_tag(node: &Handle, expected: &str) -> bool {
    matches!(&node.data, NodeData::Element { name, .. }
        if name.ns == ns!(html) && is_tag(node, expected))
}

fn load_script(loader: &HtmlSubresourceLoader, node: &Handle) -> Option<String> {
    attribute(node, "src")
        .map(|reference| load_text(loader, "script", reference).map(|(_, source)| source))
        .unwrap_or_else(|| Some(text_content(node)))
}

#[cfg(test)]
mod tests {
    use super::load_scripts;
    use crate::renderer::backends::html_browser::HtmlBrowserSource;
    use crate::renderer::backends::html_document::HtmlDocument;
    use crate::renderer::backends::html_subresources::HtmlSubresourceLoader;

    fn must_result<T, E>(result: Result<T, E>) -> T {
        assert!(result.is_ok());
        let mut values = result.into_iter().collect::<Vec<_>>();
        values.remove(0)
    }

    #[test]
    fn body_onload_after_an_inline_iframe_keeps_the_iframe_script_before_it() {
        let source = must_result(HtmlBrowserSource::new(
            r#"<iframe id=frame data-krr-local-frame></iframe><body></body><body onload="window.onload = parent">"#,
            "https://example.test/index.html",
        ));
        let loader = HtmlSubresourceLoader::new(&source);
        let mut document = HtmlDocument::parse(&source.raw_html);
        let frame = must_result(
            document
                .get_element_by_id("frame")
                .ok_or("frame must exist"),
        );
        let frame = must_result(document.node(frame));
        let mut child = HtmlDocument::parse(r#"<script id=child>window.onload = child;</script>"#);
        let child_script = must_result(
            child
                .get_element_by_id("child")
                .ok_or("child script must exist"),
        );
        let child_script = must_result(child.node(child_script));
        frame.children.borrow_mut().push(child_script);

        let (scripts, body_onload_script_index) = load_scripts(&loader, &document);

        assert_eq!(scripts, ["window.onload = child;"]);
        assert_eq!(body_onload_script_index, Some(1));
    }

    #[test]
    fn body_onload_before_an_inline_iframe_runs_before_the_iframe_script() {
        let source = must_result(HtmlBrowserSource::new(
            r#"<body onload="window.onload = body"><iframe id=frame data-krr-local-frame></iframe>"#,
            "https://example.test/index.html",
        ));
        let loader = HtmlSubresourceLoader::new(&source);
        let mut document = HtmlDocument::parse(&source.raw_html);
        let frame = must_result(
            document
                .get_element_by_id("frame")
                .ok_or("frame must exist"),
        );
        let frame = must_result(document.node(frame));
        let mut child = HtmlDocument::parse(r#"<script id=child>window.onload = child;</script>"#);
        let child_script = must_result(
            child
                .get_element_by_id("child")
                .ok_or("child script must exist"),
        );
        let child_script = must_result(child.node(child_script));
        frame.children.borrow_mut().push(child_script);

        let (scripts, body_onload_script_index) = load_scripts(&loader, &document);

        assert_eq!(scripts, ["window.onload = child;"]);
        assert_eq!(body_onload_script_index, Some(0));
    }

    #[test]
    fn body_onload_after_a_network_iframe_keeps_following_script_order() {
        let source = must_result(HtmlBrowserSource::new(
            r#"<script>before</script><iframe src="frame.html"></iframe><script>after</script><body onload="body">"#,
            "https://example.test/index.html",
        ));
        let loader = HtmlSubresourceLoader::new(&source);
        let document = HtmlDocument::parse(&source.raw_html);

        let (scripts, body_onload_script_index) = load_scripts(&loader, &document);

        assert_eq!(scripts, ["before", "after"]);
        assert_eq!(body_onload_script_index, Some(2));
    }

    #[test]
    fn foster_parented_iframe_keeps_parser_position_for_body_onload() {
        let source = must_result(HtmlBrowserSource::new(
            r#"<table><script>before</script><body onload="body"><iframe id=frame data-krr-local-frame></iframe></table>"#,
            "https://example.test/index.html",
        ));
        let loader = HtmlSubresourceLoader::new(&source);
        let mut document = HtmlDocument::parse(&source.raw_html);
        let frame = must_result(
            document
                .get_element_by_id("frame")
                .ok_or("frame must exist"),
        );
        let frame = must_result(document.node(frame));
        let mut child = HtmlDocument::parse(r#"<script id=child>frame</script>"#);
        let child_script = must_result(
            child
                .get_element_by_id("child")
                .ok_or("child script must exist"),
        );
        let child_script = must_result(child.node(child_script));
        frame.children.borrow_mut().push(child_script);

        let (scripts, body_onload_script_index) = load_scripts(&loader, &document);

        assert_eq!(scripts, ["before", "frame"]);
        assert_eq!(body_onload_script_index, Some(1));
    }

    #[test]
    fn foster_parented_iframes_keep_each_parser_position_across_parent_scripts() {
        let source = must_result(HtmlBrowserSource::new(
            r#"<iframe id=a data-krr-local-frame></iframe><table><script>parent</script><body onload="body"><iframe id=b data-krr-local-frame></iframe></table>"#,
            "https://example.test/index.html",
        ));
        let loader = HtmlSubresourceLoader::new(&source);
        let mut document = HtmlDocument::parse(&source.raw_html);
        for (id, script) in [("a", "a"), ("b", "b")] {
            let frame = must_result(document.get_element_by_id(id).ok_or("frame must exist"));
            let frame = must_result(document.node(frame));
            let mut child = HtmlDocument::parse(&format!("<script id=child>{script}</script>"));
            let child_script = must_result(
                child
                    .get_element_by_id("child")
                    .ok_or("child script must exist"),
            );
            let child_script = must_result(child.node(child_script));
            frame.children.borrow_mut().push(child_script);
        }

        let (scripts, body_onload_script_index) = load_scripts(&loader, &document);

        assert_eq!(scripts, ["a", "parent", "b"]);
        assert_eq!(body_onload_script_index, Some(2));
    }

    #[test]
    fn scripts_inside_foreign_svg_iframes_are_not_collected() {
        let source = must_result(HtmlBrowserSource::new(
            r#"<iframe id=frame data-krr-local-frame></iframe>"#,
            "https://example.test/index.html",
        ));
        let loader = HtmlSubresourceLoader::new(&source);
        let mut document = HtmlDocument::parse(&source.raw_html);
        let frame = must_result(
            document
                .get_element_by_id("frame")
                .ok_or("frame must exist"),
        );
        let frame = must_result(document.node(frame));
        let child = HtmlDocument::parse(
            r#"<svg><iframe><script>foreign</script></iframe></svg><script>html</script>"#,
        );
        let child_nodes = child.document.children.borrow().clone();
        frame.children.borrow_mut().extend(child_nodes);

        let (scripts, _) = load_scripts(&loader, &document);

        assert_eq!(scripts, ["html"]);
    }
}
