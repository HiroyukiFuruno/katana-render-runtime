use super::tree::WindowLoadHandlerObserver;
#[path = "html_document_parse_source_order_events.rs"]
mod events;
use events::{SourceOrderEntry, finish_source_order, node_is_html_select, node_is_visible};
use html5ever::{
    tokenizer::{Tag, TagKind, Token, TokenSink, TokenSinkResult},
    tree_builder::{Tracer, TreeBuilder, TreeBuilderOpts, TreeSink},
};
use markup5ever_rcdom::{Handle, RcDom};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

pub(super) struct SourceOrderSink {
    tree_builder: TreeBuilder<Handle, RcDom>,
    scripts: Cell<usize>,
    node_visibility: RefCell<HashMap<usize, bool>>,
    window_load_handler_observer: RefCell<WindowLoadHandlerObserver>,
    body_onload_script_index: Cell<Option<usize>>,
    body_onload_source_order_index: Cell<Option<usize>>,
    source_order: RefCell<Vec<SourceOrderEntry>>,
}

struct OpenSelectObserver {
    present: Cell<bool>,
}

impl Tracer for OpenSelectObserver {
    type Handle = Handle;

    fn trace_handle(&self, node: &Handle) {
        if node_is_html_select(node) {
            self.present.set(true);
        }
    }
}

impl SourceOrderSink {
    pub(super) fn new(dom: RcDom) -> Self {
        Self {
            tree_builder: TreeBuilder::new(dom, TreeBuilderOpts::default()),
            scripts: Cell::new(0),
            node_visibility: RefCell::new(HashMap::new()),
            window_load_handler_observer: RefCell::new(WindowLoadHandlerObserver::default()),
            body_onload_script_index: Cell::new(None),
            body_onload_source_order_index: Cell::new(None),
            source_order: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn finish(self) -> (RcDom, Option<usize>, Option<usize>, Vec<Handle>) {
        let parsed = self.tree_builder.sink.finish();
        let (body_onload_source_order_index, source_order) = finish_source_order(
            &parsed,
            self.source_order.into_inner(),
            self.body_onload_source_order_index.get(),
            self.body_onload_script_index.get(),
            &self.node_visibility,
        );
        (
            parsed,
            self.body_onload_script_index.get(),
            body_onload_source_order_index,
            source_order,
        )
    }

    fn observe_script(&self, node: &Handle) {
        if self.node_is_visible(node) {
            self.scripts.set(self.scripts.get() + 1);
            self.source_order
                .borrow_mut()
                .push(SourceOrderEntry::Script(node.clone()));
        }
    }

    fn observe_source_order(&self, tag: &Tag) {
        if tag.kind != TagKind::StartTag {
            return;
        }
        if tag.name.to_string().eq_ignore_ascii_case("iframe") {
            /* WHY: 文書全体のparserではselect handleをopen stackにだけ保持するため、
             * token処理後のtraceで暗黙の閉鎖を反映し、DOM全体の再走査を避ける。 */
            let observer = OpenSelectObserver {
                present: Cell::new(false),
            };
            self.tree_builder.trace_handles(&observer);
            let in_select = observer.present.get();
            self.source_order
                .borrow_mut()
                .push(SourceOrderEntry::Iframe { in_select });
        }
        self.observe_window_load_handler(tag);
    }

    fn observe_window_load_handler(&self, tag: &Tag) {
        let name = tag.name.to_string();
        let load_handler_element =
            name.eq_ignore_ascii_case("body") || name.eq_ignore_ascii_case("frameset");
        if !load_handler_element
            || self.body_onload_script_index.get().is_some()
            || !tag.attrs.iter().any(|attribute| {
                attribute
                    .name
                    .local
                    .to_string()
                    .eq_ignore_ascii_case("onload")
            })
            || !self
                .window_load_handler_observer
                .borrow_mut()
                .token_created_or_updated_window_load_handler(
                    &self.tree_builder.sink.document,
                    &name,
                )
        {
            return;
        }
        self.body_onload_script_index.set(Some(self.scripts.get()));
        self.body_onload_source_order_index
            .set(Some(self.source_order.borrow().len()));
    }

    fn node_is_visible(&self, node: &Handle) -> bool {
        node_is_visible(
            node,
            &self.tree_builder.sink.document,
            &self.node_visibility,
        )
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use html5ever::{local_name, parse_document, tendril::TendrilSink, tokenizer::TagKind};

    #[test]
    fn iframe_start_without_a_tree_node_does_not_emit_source_order_entry() {
        let sink = SourceOrderSink::new(RcDom::default());
        sink.observe_source_order(&Tag {
            kind: TagKind::StartTag,
            name: local_name!(iframe),
            self_closing: false,
            attrs: Vec::new(),
            had_duplicate_attributes: false,
        });

        let (_, _, _, source_order) = sink.finish();
        assert!(source_order.is_empty());
    }

    #[test]
    fn iframe_token_in_select_is_skipped_when_collected_iframe_is_outside_select() {
        let document =
            parse_document(RcDom::default(), Default::default()).one("<iframe></iframe>");
        let visibility = RefCell::new(HashMap::new());

        let (_, source_order) = events::finish_source_order(
            &document,
            vec![events::SourceOrderEntry::Iframe { in_select: true }],
            None,
            None,
            &visibility,
        );

        assert!(source_order.is_empty());
    }
}
