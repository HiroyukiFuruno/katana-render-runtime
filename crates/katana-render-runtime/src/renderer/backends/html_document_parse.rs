use super::super::html_dom_helpers::collect_scripts;
use super::HtmlDocument;
#[path = "html_document_parse_source_order.rs"]
mod source_order;
#[path = "html_document_parse_tree.rs"]
mod tree;
use html5ever::{
    TokenizerResult,
    tokenizer::{BufferQueue, Tokenizer, TokenizerOpts},
};
use markup5ever_rcdom::{Handle, RcDom};
use source_order::SourceOrderSink;
use std::collections::HashMap;

impl HtmlDocument {
    pub(crate) fn parse(source: &str) -> Self {
        let (parsed, body_onload_script_index, body_onload_source_order_index, source_order) =
            parse_with_source_order(source);
        let mut document = Self {
            document: parsed.document,
            body_onload_script_index,
            source_order,
            body_onload_source_order_index,
            nodes: HashMap::new(),
            node_ids: HashMap::new(),
            next_node_id: 1,
        };
        document.register_subtree(&document.document.clone());
        document
    }

    pub(crate) fn render(&self) -> String {
        super::super::html_snapshot::render_document(&self.document)
    }

    pub(crate) fn inline_scripts(&self) -> Result<Vec<String>, String> {
        let mut scripts = Vec::new();
        collect_scripts(&self.document, &mut scripts)?;
        Ok(scripts)
    }

    /// body または frameset の onload 属性へ到達する前に実行される inline script 数を返す。
    ///
    /// body.onload と frameset.onload は Window の load プロパティと共有するため、V8 の初期化時点では
    /// なく、HTML パーサが対応する開始タグを処理する順序で導入しなければならない。
    pub(crate) fn body_onload_script_index(&self) -> Option<usize> {
        self.body_onload_script_index
    }
}

fn parse_with_source_order(source: &str) -> (RcDom, Option<usize>, Option<usize>, Vec<Handle>) {
    let sink = SourceOrderSink::new(RcDom::default());
    let tokenizer = Tokenizer::new(sink, TokenizerOpts::default());
    let input = BufferQueue::default();
    input.push_back(source.to_string().into());
    loop {
        if matches!(tokenizer.feed(&input), TokenizerResult::Done) {
            break;
        }
    }
    tokenizer.end();
    tokenizer.sink.finish()
}

#[cfg(test)]
mod tests {
    use super::HtmlDocument;

    #[test]
    fn body_onload_uses_later_duplicate_body_token_position() {
        let document = HtmlDocument::parse(
            r#"<body><script>window.onload=first</script><body onload="second">"#,
        );

        assert_eq!(document.body_onload_script_index(), Some(1));
    }

    #[test]
    fn frameset_onload_uses_its_source_position_for_the_window_load_handler() {
        let document = HtmlDocument::parse(
            r#"<head><script>headScript()</script></head><frameset onload="handler"><script>framesetScript()</script></frameset>"#,
        );

        assert_eq!(document.body_onload_script_index(), Some(1));
    }

    #[test]
    fn foreign_svg_frameset_onload_does_not_set_the_window_load_handler_position() {
        let document = HtmlDocument::parse(
            r#"<script>firstScript()</script><svg><frameset onload="foreignHandler"></frameset></svg><script>secondScript()</script><body onload="handler">"#,
        );

        assert_eq!(document.body_onload_script_index(), Some(2));
    }

    #[test]
    fn script_markup_inside_title_rcdata_does_not_shift_body_onload_position() {
        let document = HtmlDocument::parse(
            r#"<title><script>not a script</script></title><body onload="handler"><script>realScript()</script>"#,
        );

        assert_eq!(document.body_onload_script_index(), Some(0));
    }

    #[test]
    fn script_inside_template_does_not_shift_body_onload_position() {
        let document = HtmlDocument::parse(
            r#"<head><script>headScript()</script></head><template><script>templateScript()</script></template><body onload="handler"><script>bodyScript()</script>"#,
        );

        assert_eq!(document.body_onload_script_index(), Some(1));
    }

    #[test]
    fn iframe_inside_template_does_not_shift_body_onload_position() {
        let document = HtmlDocument::parse(
            r#"<template><iframe></iframe></template><body onload="handler"><iframe></iframe>"#,
        );

        assert_eq!(document.body_onload_source_order_index, Some(0));
    }

    #[test]
    fn visible_iframe_before_body_onload_shifts_its_source_position() {
        let document = HtmlDocument::parse(r#"<iframe></iframe><body onload="handler">"#);

        assert_eq!(document.body_onload_source_order_index, Some(1));
    }

    #[test]
    fn iframe_after_closed_select_keeps_source_order() {
        let document = HtmlDocument::parse(
            r#"<select><iframe></iframe></select><script>afterSelect()</script><body onload="handler"><iframe></iframe>"#,
        );

        assert_eq!(document.body_onload_script_index(), Some(1));
        assert_eq!(document.body_onload_source_order_index, Some(1));
        assert_eq!(document.source_order.len(), 2);
    }

    #[test]
    fn iframe_after_implicitly_closed_select_keeps_source_order() {
        let document = HtmlDocument::parse(
            r#"<select><input><iframe></iframe><script>afterSelect()</script><body onload="handler"><iframe></iframe>"#,
        );

        assert_eq!(document.body_onload_script_index(), Some(1));
        assert_eq!(document.body_onload_source_order_index, Some(2));
        assert_eq!(document.source_order.len(), 3);
    }

    #[test]
    fn iframe_after_nested_select_keeps_source_order() {
        let document = HtmlDocument::parse(
            r#"<select><select><iframe></iframe><script>afterSelect()</script><body onload="handler"><iframe></iframe>"#,
        );

        assert_eq!(document.body_onload_script_index(), Some(1));
        assert_eq!(document.body_onload_source_order_index, Some(2));
        assert_eq!(document.source_order.len(), 3);
    }

    #[test]
    fn iframe_after_closed_nested_select_keeps_source_order() {
        let document = HtmlDocument::parse(
            r#"<select><select></select><iframe></iframe><script>afterSelect()</script><body onload="handler"><iframe></iframe>"#,
        );

        assert_eq!(document.body_onload_script_index(), Some(1));
        assert_eq!(document.body_onload_source_order_index, Some(2));
        assert_eq!(document.source_order.len(), 3);
    }

    #[test]
    fn iframe_after_ignored_select_iframe_keeps_parser_order() {
        let document = HtmlDocument::parse(
            r#"<select><iframe></iframe></select><script>afterSelect()</script><body onload="handler"><iframe id=real>"#,
        );

        let real_index = document.source_order.iter().position(|node| {
            matches!(
                &node.data,
                markup5ever_rcdom::NodeData::Element { attrs, .. }
                    if attrs.borrow().iter().any(|attribute| {
                        attribute.name.local.as_str() == "id"
                            && attribute.value.as_ref() == "real"
                    })
            )
        });
        assert_eq!(document.body_onload_source_order_index, Some(1));
        assert_eq!(real_index, Some(1));
    }

    #[test]
    fn multiple_ignored_select_iframes_keep_following_iframe_order() {
        let document = HtmlDocument::parse(
            r#"<select><iframe></iframe><iframe></iframe></select><script>afterSelect()</script><body onload="handler"><iframe id=real>"#,
        );

        let real_index = document.source_order.iter().position(|node| {
            matches!(
                &node.data,
                markup5ever_rcdom::NodeData::Element { attrs, .. }
                    if attrs.borrow().iter().any(|attribute| {
                        attribute.name.local.as_str() == "id"
                            && attribute.value.as_ref() == "real"
                    })
            )
        });
        assert_eq!(document.body_onload_source_order_index, Some(1));
        assert_eq!(real_index, Some(1));
    }

    #[test]
    fn self_closing_foreign_template_does_not_hide_body_onload() {
        let document = HtmlDocument::parse(
            r#"<svg><template/></svg><body onload="handler"><script>bodyScript()</script>"#,
        );

        assert_eq!(document.body_onload_script_index(), Some(0));
    }

    #[test]
    fn foreign_template_without_self_closing_does_not_hide_body_onload() {
        let document = HtmlDocument::parse(
            r#"<svg><template></svg><body onload="handler"><script>bodyScript()</script>"#,
        );

        assert_eq!(document.body_onload_script_index(), Some(0));
    }

    #[test]
    fn implicit_foreign_template_pop_does_not_hide_body_onload() {
        let document = HtmlDocument::parse(
            r#"<template><svg><template></svg></template><body onload="handler()"><script>bodyScript()</script>"#,
        );

        assert_eq!(document.body_onload_script_index(), Some(0));
    }

    #[test]
    fn many_visible_scripts_keep_body_onload_index_without_rescanning_the_document() {
        const SCRIPT_COUNT: usize = 50_000;
        let mut source = String::from("<body>");
        source.push_str(&"<script></script>".repeat(SCRIPT_COUNT));
        source.push_str(r#"<body onload="handler()">"#);
        assert!(source.len() < 16 * 1024 * 1024);

        let document = HtmlDocument::parse(&source);

        assert_eq!(document.body_onload_script_index(), Some(SCRIPT_COUNT));
    }

    #[test]
    fn repeated_foreign_body_and_frameset_tokens_do_not_rescan_html_siblings() {
        const TOKEN_COUNT: usize = 8_000;
        let mut source = String::from("<html>");
        source.push_str(&"<!-- sibling noise -->".repeat(TOKEN_COUNT));
        source.push_str("<head></head><body><script>beforeHandler()</script><svg>");
        for _ in 0..TOKEN_COUNT {
            source.push_str(r#"<body onload="foreignBody"/><frameset onload="foreignFrameset"/>"#);
        }
        source.push_str(r#"</svg><body onload="handler">"#);

        let document = HtmlDocument::parse(&source);

        assert_eq!(document.body_onload_script_index(), Some(1));
    }
}
