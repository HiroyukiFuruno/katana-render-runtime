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
            prevalidated_image_events: HashMap::new(),
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
    use markup5ever_rcdom::{Handle, NodeData};
    use std::rc::Rc;

    const SELECT_TRANSITION_CASES: [(&str, &str); 6] = [
        (
            "table row",
            "<table><select><tr><td><iframe id=real></iframe></td></tr></table>",
        ),
        (
            "table section",
            "<table><select><tbody><tr><td><iframe id=real></iframe></td></tr></tbody></table>",
        ),
        (
            "table end",
            "<table><select></table><iframe id=real></iframe>",
        ),
        ("nested select", "<select><select><iframe id=real></iframe>"),
        (
            "closed nested select",
            "<select><select></select><iframe id=real></iframe>",
        ),
        (
            "foreign select",
            "<svg><select><foreignObject><iframe id=real></iframe></foreignObject></select></svg>",
        ),
    ];

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
    fn actual_select_parser_state_preserves_iframe_source_order() -> Result<(), String> {
        for (case, prefix) in SELECT_TRANSITION_CASES {
            let mut document = HtmlDocument::parse(&format!(
                "{prefix}<script id=after>afterSelect()</script><body onload=handler><iframe id=tail></iframe>"
            ));
            let real = actual_element(&mut document, "real")?;
            assert_html_iframe(&real, case);
            assert!(!has_html_select_ancestor(&real), "{case}");
            let after = actual_element(&mut document, "after")?;
            let tail = actual_element(&mut document, "tail")?;
            assert_eq!(document.source_order.len(), 3, "{case}");
            for (index, node) in [real, after, tail].iter().enumerate() {
                assert!(
                    Rc::ptr_eq(&document.source_order[index], node),
                    "{case}: {index}"
                );
            }
            assert_eq!(document.body_onload_source_order_index, Some(2), "{case}");
        }
        Ok(())
    }

    #[test]
    fn actual_html_select_ancestor_sets_positive_branch() -> Result<(), String> {
        let mut document =
            HtmlDocument::parse("<select id=outer><option id=option>choice</option></select>");
        let select = actual_element(&mut document, "outer")?;
        let option = actual_element(&mut document, "option")?;

        assert!(!has_html_select_ancestor(&select));
        assert!(has_html_select_ancestor(&option));
        Ok(())
    }

    #[test]
    fn actual_element_reports_missing_id() -> Result<(), String> {
        let mut document = HtmlDocument::parse("");
        let result = actual_element(&mut document, "missing");

        assert!(matches!(
            result,
            Err(error) if error == "actual DOM element #missing is missing"
        ));
        Ok(())
    }

    fn actual_element(document: &mut HtmlDocument, id: &str) -> Result<Handle, String> {
        let node_id = document
            .get_element_by_id(id)
            .ok_or_else(|| format!("actual DOM element #{id} is missing"))?;
        document.node(node_id)
    }

    fn assert_html_iframe(node: &Handle, case: &str) {
        assert!(
            matches!(
                &node.data,
                NodeData::Element { name, .. }
                    if name.ns.as_str() == "http://www.w3.org/1999/xhtml"
                        && name.local.as_str() == "iframe"
            ),
            "actual DOM iframe must be HTML: {case}"
        );
    }

    fn has_html_select_ancestor(node: &Handle) -> bool {
        let mut current = node.clone();
        loop {
            let parent = current.parent.take();
            current.parent.set(parent.clone());
            let Some(parent) = parent.and_then(|parent| parent.upgrade()) else {
                return false;
            };
            if matches!(
                &parent.data,
                NodeData::Element { name, .. }
                    if name.ns.as_str() == "http://www.w3.org/1999/xhtml"
                        && name.local.as_str() == "select"
            ) {
                return true;
            }
            current = parent;
        }
    }

    #[test]
    fn iframe_after_ignored_select_iframe_keeps_parser_order() -> Result<(), String> {
        let mut document = HtmlDocument::parse(
            r#"<select><iframe id=hidden></iframe></select><script>afterSelect()</script><body onload="handler"><iframe id=real>"#,
        );

        let hidden = actual_element(&mut document, "hidden")?;
        assert_html_iframe(&hidden, "select child");
        assert!(has_html_select_ancestor(&hidden));
        assert!(
            !document
                .source_order
                .iter()
                .any(|node| Rc::ptr_eq(node, &hidden))
        );
        let real = actual_element(&mut document, "real")?;
        assert_html_iframe(&real, "after select");
        assert!(!has_html_select_ancestor(&real));
        let real_index = document
            .source_order
            .iter()
            .position(|node| Rc::ptr_eq(node, &real));
        assert_eq!(document.body_onload_source_order_index, Some(1));
        assert_eq!(real_index, Some(1));
        Ok(())
    }

    #[test]
    fn multiple_ignored_select_iframes_keep_following_iframe_order() -> Result<(), String> {
        let mut document = HtmlDocument::parse(
            r#"<select><iframe id=hidden1></iframe><iframe id=hidden2></iframe></select><script>afterSelect()</script><body onload="handler"><iframe id=real>"#,
        );

        for id in ["hidden1", "hidden2"] {
            let hidden = actual_element(&mut document, id)?;
            assert_html_iframe(&hidden, id);
            assert!(has_html_select_ancestor(&hidden));
            assert!(
                !document
                    .source_order
                    .iter()
                    .any(|node| Rc::ptr_eq(node, &hidden))
            );
        }
        let real = actual_element(&mut document, "real")?;
        assert_html_iframe(&real, "after select");
        assert!(!has_html_select_ancestor(&real));
        let real_index = document
            .source_order
            .iter()
            .position(|node| Rc::ptr_eq(node, &real));
        assert_eq!(document.body_onload_source_order_index, Some(1));
        assert_eq!(real_index, Some(1));
        Ok(())
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
