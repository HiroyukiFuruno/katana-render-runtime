use super::tree::WindowLoadHandlerObserver;
use html5ever::{
    tokenizer::{Tag, TagKind, Token, TokenSink, TokenSinkResult},
    tree_builder::{TreeBuilder, TreeBuilderOpts, TreeSink},
};
use markup5ever_rcdom::{Handle, RcDom};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

pub(super) struct SourceOrderSink {
    tree_builder: TreeBuilder<Handle, RcDom>,
    scripts: Cell<usize>,
    node_visibility: RefCell<HashMap<usize, bool>>,
    window_load_handler_observer: RefCell<WindowLoadHandlerObserver>,
    body_onload_script_index: Cell<Option<usize>>,
    body_onload_source_order_index: Cell<Option<usize>>,
    source_order: RefCell<Vec<Handle>>,
    source_iframes: RefCell<HashSet<usize>>,
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
            source_iframes: RefCell::new(HashSet::new()),
        }
    }

    pub(super) fn finish(self) -> (RcDom, Option<usize>, Option<usize>, Vec<Handle>) {
        let body_onload_script_index = self.body_onload_script_index.get();
        let body_onload_source_order_index = self.body_onload_source_order_index.get();
        let source_order = self.source_order.into_inner();
        let parsed = self.tree_builder.sink.finish();
        (
            parsed,
            body_onload_script_index,
            body_onload_source_order_index,
            source_order,
        )
    }

    fn observe_script(&self, node: &Handle) {
        if self.node_is_visible(node) {
            self.scripts.set(self.scripts.get() + 1);
            self.source_order.borrow_mut().push(node.clone());
        }
    }

    fn observe_source_order(&self, tag: &Tag) {
        if tag.kind != TagKind::StartTag {
            return;
        }
        self.observe_iframe(tag);
        self.observe_window_load_handler(tag);
    }

    fn observe_iframe(&self, tag: &Tag) {
        if !tag.name.to_string().eq_ignore_ascii_case("iframe") {
            return;
        }
        let mut iframes = Vec::new();
        collect_iframes(&self.tree_builder.sink.document, &mut iframes);
        let mut seen = self.source_iframes.borrow_mut();
        let mut source_order = self.source_order.borrow_mut();
        if let Some(iframe) = iframes.into_iter().find(|iframe| {
            let id = Rc::as_ptr(iframe) as usize;
            !seen.contains(&id) && self.node_is_visible(iframe)
        }) {
            seen.insert(Rc::as_ptr(&iframe) as usize);
            source_order.push(iframe);
        }
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

fn collect_iframes(node: &Handle, iframes: &mut Vec<Handle>) {
    if matches!(&node.data, markup5ever_rcdom::NodeData::Element { name, .. }
        if name.local.as_str().eq_ignore_ascii_case("iframe"))
    {
        iframes.push(node.clone());
    }
    let Ok(children) = node.children.try_borrow() else {
        return;
    };
    for child in children.iter() {
        collect_iframes(child, iframes);
    }
}
