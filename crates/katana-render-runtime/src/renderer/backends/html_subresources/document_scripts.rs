use super::HtmlSubresourceLoader;
use super::document::{attribute, is_tag, load_text, text_content};
use crate::renderer::backends::html_document::HtmlDocument;
use markup5ever_rcdom::Handle;

pub(super) fn load_scripts(
    loader: &HtmlSubresourceLoader,
    document: &HtmlDocument,
) -> (Vec<String>, Option<usize>) {
    let mut collector = ScriptCollector::new(
        loader,
        document.body_onload_script_index(),
        document.body_onload_iframe_index(),
    );
    collector.collect(&document.document);
    collector.finish()
}

struct ScriptCollector<'a> {
    loader: &'a HtmlSubresourceLoader,
    scripts: Vec<String>,
    body_source_script_index: Option<usize>,
    body_source_iframe_index: Option<usize>,
    source_script_index: usize,
    source_iframe_index: usize,
    iframe_script_counts: Vec<usize>,
    body_onload_script_index: Option<usize>,
}

impl<'a> ScriptCollector<'a> {
    fn new(
        loader: &'a HtmlSubresourceLoader,
        body_source_script_index: Option<usize>,
        body_source_iframe_index: Option<usize>,
    ) -> Self {
        Self {
            loader,
            scripts: Vec::new(),
            body_source_script_index,
            body_source_iframe_index,
            source_script_index: 0,
            source_iframe_index: 0,
            iframe_script_counts: Vec::new(),
            body_onload_script_index: None,
        }
    }

    fn collect(&mut self, node: &Handle) {
        self.collect_node(node, false);
    }

    fn collect_node(&mut self, node: &Handle, inside_inline_frame: bool) {
        self.record_body_onload_script_index();
        if is_tag(node, "script") {
            self.collect_script(node, inside_inline_frame);
            return;
        }
        let counts_source_iframe = !inside_inline_frame && is_tag(node, "iframe");
        let iframe_script_start = self.scripts.len();
        let inside_inline_frame = inside_inline_frame || is_inline_frame(node);
        for child in node.children.borrow().iter() {
            self.collect_node(child, inside_inline_frame);
        }
        if counts_source_iframe {
            self.iframe_script_counts
                .push(self.scripts.len() - iframe_script_start);
            self.source_iframe_index += 1;
            self.record_body_onload_script_index();
        }
    }

    fn collect_script(&mut self, node: &Handle, inside_inline_frame: bool) {
        if !inside_inline_frame {
            self.record_body_onload_script_index();
            self.source_script_index += 1;
        }
        if let Some(script) = load_script(self.loader, node) {
            self.scripts.push(script);
        }
        if !inside_inline_frame {
            self.record_body_onload_script_index();
        }
    }

    fn record_body_onload_script_index(&mut self) {
        if self.body_onload_script_index.is_none()
            && self.body_source_script_index == Some(self.source_script_index)
            && self
                .body_source_iframe_index
                .is_some_and(|index| self.source_iframe_index >= index)
        {
            let deferred_iframe_scripts = if let Some(index) = self.body_source_iframe_index {
                self.iframe_script_counts.iter().skip(index).sum::<usize>()
            } else {
                0
            };
            self.body_onload_script_index = Some(self.scripts.len() - deferred_iframe_scripts);
        }
    }

    fn finish(mut self) -> (Vec<String>, Option<usize>) {
        if self.body_onload_script_index.is_none()
            && self.body_source_script_index == Some(self.source_script_index)
            && self
                .body_source_iframe_index
                .is_some_and(|index| self.source_iframe_index >= index)
        {
            let deferred_iframe_scripts = if let Some(index) = self.body_source_iframe_index {
                self.iframe_script_counts.iter().skip(index).sum::<usize>()
            } else {
                0
            };
            self.body_onload_script_index = Some(self.scripts.len() - deferred_iframe_scripts);
        }
        (self.scripts, self.body_onload_script_index)
    }
}

fn is_inline_frame(node: &Handle) -> bool {
    is_tag(node, "iframe") && attribute(node, "data-krr-local-frame").is_some()
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

        assert_eq!(scripts, ["frame", "before"]);
        assert_eq!(body_onload_script_index, Some(1));
    }
}
