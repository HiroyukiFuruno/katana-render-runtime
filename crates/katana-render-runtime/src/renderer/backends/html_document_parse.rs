use super::super::html_dom_helpers::collect_scripts;
use super::HtmlDocument;
#[path = "html_document_parse_tree.rs"]
mod tree;
use html5ever::{
    TokenizerResult,
    tokenizer::{
        BufferQueue, Tag, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
    },
    tree_builder::{TreeBuilder, TreeBuilderOpts, TreeSink},
};
use markup5ever_rcdom::{Handle, RcDom};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
use tree::{WindowLoadHandlerObserver, visible_iframe_count};

impl HtmlDocument {
    pub(crate) fn parse(source: &str) -> Self {
        let (parsed, body_onload_script_index, body_onload_iframe_index) =
            parse_with_source_order(source);
        let mut document = Self {
            document: parsed.document,
            body_onload_script_index,
            body_onload_iframe_index,
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

    pub(crate) fn body_onload_iframe_index(&self) -> Option<usize> {
        self.body_onload_iframe_index
    }
}

struct SourceOrderSink {
    tree_builder: TreeBuilder<Handle, RcDom>,
    scripts: Cell<usize>,
    node_visibility: RefCell<HashMap<usize, bool>>,
    window_load_handler_observer: RefCell<WindowLoadHandlerObserver>,
    body_onload_script_index: Cell<Option<usize>>,
    body_onload_iframe_index: Cell<Option<usize>>,
}

impl TokenSink for SourceOrderSink {
    type Handle = Handle;

    fn process_token(&self, token: Token, line_number: u64) -> TokenSinkResult<Self::Handle> {
        let tag = match &token {
            Token::TagToken(tag) => Some(tag.clone()),
            _ => None,
        };
        let result = self.tree_builder.process_token(token, line_number);
        if let TokenSinkResult::Script(node) = &result {
            self.observe_script(node);
        }
        if let Some(tag) = tag {
            self.observe_source_order(&tag);
        }
        result
    }
}

impl SourceOrderSink {
    fn observe_script(&self, node: &Handle) {
        if self.node_is_visible(node) {
            self.scripts.set(self.scripts.get() + 1);
        }
    }

    fn observe_source_order(&self, tag: &Tag) {
        if tag.kind != TagKind::StartTag {
            return;
        }
        let name = tag.name.to_string();
        let load_handler_element =
            name.eq_ignore_ascii_case("body") || name.eq_ignore_ascii_case("frameset");
        if load_handler_element
            && self.body_onload_script_index.get().is_none()
            && tag.attrs.iter().any(|attribute| {
                attribute
                    .name
                    .local
                    .to_string()
                    .eq_ignore_ascii_case("onload")
            })
            && self
                .window_load_handler_observer
                .borrow_mut()
                .token_created_or_updated_window_load_handler(
                    &self.tree_builder.sink.document,
                    &name,
                )
        {
            self.body_onload_script_index.set(Some(self.scripts.get()));
            self.body_onload_iframe_index
                .set(Some(visible_iframe_count(&self.tree_builder.sink.document)));
        }
    }

    fn node_is_visible(&self, node: &Handle) -> bool {
        let mut current = node.clone();
        let mut unobserved = Vec::new();
        let visible = loop {
            let node_id = Rc::as_ptr(&current) as usize;
            if let Some(visible) = self.node_visibility.borrow().get(&node_id) {
                break *visible;
            }
            unobserved.push(node_id);
            let parent = current.parent.take();
            current.parent.set(parent.clone());
            let Some(parent) = parent.and_then(|parent| parent.upgrade()) else {
                break Rc::ptr_eq(&current, &self.tree_builder.sink.document);
            };
            current = parent;
        };
        let mut visibility = self.node_visibility.borrow_mut();
        for node_id in unobserved {
            visibility.insert(node_id, visible);
        }
        visible
    }
}

fn parse_with_source_order(source: &str) -> (RcDom, Option<usize>, Option<usize>) {
    let sink = SourceOrderSink {
        tree_builder: TreeBuilder::new(RcDom::default(), TreeBuilderOpts::default()),
        scripts: Cell::new(0),
        node_visibility: RefCell::new(HashMap::new()),
        window_load_handler_observer: RefCell::new(WindowLoadHandlerObserver::default()),
        body_onload_script_index: Cell::new(None),
        body_onload_iframe_index: Cell::new(None),
    };
    let tokenizer = Tokenizer::new(sink, TokenizerOpts::default());
    let input = BufferQueue::default();
    input.push_back(source.to_string().into());
    loop {
        if matches!(tokenizer.feed(&input), TokenizerResult::Done) {
            break;
        }
    }
    tokenizer.end();
    let body_onload_script_index = tokenizer.sink.body_onload_script_index.get();
    let body_onload_iframe_index = tokenizer.sink.body_onload_iframe_index.get();
    let parsed = tokenizer.sink.tree_builder.sink.finish();
    (parsed, body_onload_script_index, body_onload_iframe_index)
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

        assert_eq!(document.body_onload_iframe_index(), Some(0));
    }

    #[test]
    fn visible_iframe_before_body_onload_shifts_its_source_position() {
        let document = HtmlDocument::parse(r#"<iframe></iframe><body onload="handler">"#);

        assert_eq!(document.body_onload_iframe_index(), Some(1));
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
