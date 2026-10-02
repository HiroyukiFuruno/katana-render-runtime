use super::super::super::LaterBodyOnloadToken;
use markup5ever_rcdom::{Handle, RcDom};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub(super) enum SourceOrderEntry {
    Script(Handle),
    Iframe { in_select: bool },
}

struct SourceOrderReplay<'a> {
    iframes: &'a [(Handle, bool, bool)],
    iframe_index: usize,
    document: &'a Handle,
    node_visibility: &'a RefCell<HashMap<usize, bool>>,
}

pub(super) fn finish_source_order(
    parsed: &RcDom,
    entries: Vec<SourceOrderEntry>,
    body_onload_event_index: Option<usize>,
    body_onload_script_index: Option<usize>,
    node_visibility: &RefCell<HashMap<usize, bool>>,
    later_body_onload_tokens: &mut [LaterBodyOnloadToken],
) -> (Option<usize>, Vec<Handle>) {
    let mut iframes = Vec::new();
    collect_iframes(&parsed.document, &mut iframes, false, false);
    let (mut body_onload_source_order_index, source_order) = replay_source_order(
        entries,
        SourceOrderReplay {
            iframes: &iframes,
            iframe_index: 0,
            document: &parsed.document,
            node_visibility,
        },
        body_onload_event_index,
        later_body_onload_tokens,
    );
    if body_onload_source_order_index.is_none() && body_onload_script_index.is_some() {
        body_onload_source_order_index = Some(source_order.len());
    }
    (body_onload_source_order_index, source_order)
}

fn replay_source_order(
    entries: Vec<SourceOrderEntry>,
    mut replay: SourceOrderReplay<'_>,
    body_onload_event_index: Option<usize>,
    later_body_onload_tokens: &mut [LaterBodyOnloadToken],
) -> (Option<usize>, Vec<Handle>) {
    let mut source_order = Vec::with_capacity(entries.len());
    let mut body_onload_source_order_index = None;
    let mut later_tokens = later_body_onload_tokens.iter_mut().peekable();
    for (event_index, entry) in entries.into_iter().enumerate() {
        LaterBodyOnloadToken::record_source_order(
            &mut later_tokens,
            event_index,
            source_order.len(),
        );
        if body_onload_event_index == Some(event_index) {
            body_onload_source_order_index = Some(source_order.len());
        }
        replay.append(entry, &mut source_order);
    }
    for token in later_tokens {
        token.source_order_index = source_order.len();
    }
    (body_onload_source_order_index, source_order)
}

impl SourceOrderReplay<'_> {
    fn append(&mut self, entry: SourceOrderEntry, source_order: &mut Vec<Handle>) {
        match entry {
            SourceOrderEntry::Script(script) => source_order.push(script),
            SourceOrderEntry::Iframe { in_select } => append_iframe(
                source_order,
                self.iframes,
                &mut self.iframe_index,
                in_select,
                self.document,
                self.node_visibility,
            ),
        }
    }
}

fn append_iframe(
    source_order: &mut Vec<Handle>,
    iframes: &[(Handle, bool, bool)],
    iframe_index: &mut usize,
    token_in_select: bool,
    document: &Handle,
    node_visibility: &RefCell<HashMap<usize, bool>>,
) {
    let Some((iframe, in_template, in_select)) = iframes.get(*iframe_index) else {
        return;
    };
    if token_in_select && !in_select {
        return;
    }
    *iframe_index += 1;
    if *in_template || *in_select || !node_is_visible(iframe, document, node_visibility) {
        return;
    }
    source_order.push(iframe.clone());
}

pub(super) fn node_is_visible(
    node: &Handle,
    document: &Handle,
    node_visibility: &RefCell<HashMap<usize, bool>>,
) -> bool {
    let mut current = node.clone();
    let mut unobserved = Vec::new();
    let visible = loop {
        let node_id = Rc::as_ptr(&current) as usize;
        if let Some(visible) = node_visibility.borrow().get(&node_id) {
            break *visible;
        }
        unobserved.push(node_id);
        let parent = current.parent.take();
        current.parent.set(parent.clone());
        let Some(parent) = parent.and_then(|parent| parent.upgrade()) else {
            break Rc::ptr_eq(&current, document);
        };
        current = parent;
    };
    let mut visibility = node_visibility.borrow_mut();
    for node_id in unobserved {
        visibility.insert(node_id, visible);
    }
    visible
}

fn collect_iframes(
    node: &Handle,
    iframes: &mut Vec<(Handle, bool, bool)>,
    in_template: bool,
    in_select: bool,
) {
    let (in_template, in_select) = iframe_context(node, in_template, in_select);
    collect_iframe_node(node, iframes, in_template, in_select);
}

fn iframe_context(node: &Handle, in_template: bool, in_select: bool) -> (bool, bool) {
    (
        in_template || is_html_template(node),
        in_select || node_is_html_select(node),
    )
}

fn is_html_template(node: &Handle) -> bool {
    matches!(&node.data, markup5ever_rcdom::NodeData::Element { name, .. }
        if name.ns.as_str() == "http://www.w3.org/1999/xhtml"
            && name.local.as_str() == "template")
}

pub(super) fn node_is_html_select(node: &Handle) -> bool {
    matches!(&node.data, markup5ever_rcdom::NodeData::Element { name, .. }
        if name.ns.as_str() == "http://www.w3.org/1999/xhtml"
            && name.local.as_str() == "select")
}

fn collect_iframe_node(
    node: &Handle,
    iframes: &mut Vec<(Handle, bool, bool)>,
    in_template: bool,
    in_select: bool,
) {
    if is_element_named(node, "iframe") {
        iframes.push((node.clone(), in_template, in_select));
    }
    let Ok(children) = node.children.try_borrow() else {
        return;
    };
    for child in children.iter() {
        collect_iframes(child, iframes, in_template, in_select);
    }
    if let markup5ever_rcdom::NodeData::Element {
        template_contents, ..
    } = &node.data
        && let Some(template_contents) = template_contents.borrow().as_ref()
    {
        collect_iframes(template_contents, iframes, true, in_select);
    }
}

fn is_element_named(node: &Handle, expected_name: &str) -> bool {
    matches!(&node.data, markup5ever_rcdom::NodeData::Element { name, .. }
        if name.local.as_str().eq_ignore_ascii_case(expected_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use html5ever::{parse_document, tendril::TendrilSink};

    fn element_by_id(node: &Handle, id: &str) -> Option<Handle> {
        if let markup5ever_rcdom::NodeData::Element { attrs, .. } = &node.data
            && attrs.borrow().iter().any(|attribute| {
                attribute.name.local.as_str() == "id" && attribute.value.as_ref() == id
            })
        {
            return Some(node.clone());
        }
        let children = node.children.borrow();
        for child in children.iter() {
            if let Some(found) = element_by_id(child, id) {
                return Some(found);
            }
        }
        drop(children);
        if let markup5ever_rcdom::NodeData::Element {
            template_contents, ..
        } = &node.data
            && let Some(template_contents) = template_contents.borrow().as_ref()
        {
            return element_by_id(template_contents, id);
        }
        None
    }

    fn required_element_by_id(node: &Handle, id: &str) -> Result<Handle, String> {
        element_by_id(node, id).ok_or_else(|| format!("element #{id} is missing"))
    }

    fn replay_test_source_order(parsed: &RcDom, entries: Vec<SourceOrderEntry>) -> Vec<Handle> {
        finish_source_order(
            parsed,
            entries,
            None,
            None,
            &RefCell::new(HashMap::new()),
            &mut [],
        )
        .1
    }

    #[test]
    fn no_iframe_document_has_no_collected_iframes() {
        let document = RcDom::default();
        let mut iframes = Vec::new();

        collect_iframes(&document.document, &mut iframes, false, false);
        assert!(iframes.is_empty());
    }

    #[test]
    fn source_order_ignores_iframe_entries_without_matching_nodes() {
        let document = RcDom::default();
        let visibility = RefCell::new(HashMap::new());

        let (_, source_order) = finish_source_order(
            &document,
            vec![SourceOrderEntry::Iframe { in_select: false }],
            None,
            None,
            &visibility,
            &mut [],
        );

        assert!(source_order.is_empty());
    }

    #[test]
    fn body_onload_script_without_an_event_uses_source_order_fallback_index() {
        let document = RcDom::default();
        let visibility = RefCell::new(HashMap::new());

        let (body_onload_index, source_order) =
            finish_source_order(&document, Vec::new(), None, Some(0), &visibility, &mut []);

        assert_eq!(body_onload_index, Some(0));
        assert!(source_order.is_empty());
    }

    #[test]
    fn iframe_collection_stops_when_children_are_already_borrowed() {
        let document = RcDom::default();
        let _children = document.document.children.borrow_mut();
        let mut iframes = Vec::new();

        collect_iframes(&document.document, &mut iframes, false, false);

        assert!(iframes.is_empty());
    }

    #[test]
    fn required_element_by_id_reports_missing_id() -> Result<(), String> {
        let document = RcDom::default();
        let missing = required_element_by_id(&document.document, "missing");

        assert!(matches!(missing, Err(error) if error == "element #missing is missing"));
        Ok(())
    }

    #[test]
    fn svg_template_keeps_foreign_object_iframe_in_source_order() -> Result<(), String> {
        let document = parse_document(RcDom::default(), Default::default()).one(
            r#"<script id=before>before()</script><svg><template><foreignObject><iframe id=frame></iframe><script id=inside>inside()</script></foreignObject></template></svg><script id=after>after()</script>"#,
        );
        let before = required_element_by_id(&document.document, "before")?;
        let frame = required_element_by_id(&document.document, "frame")?;
        let inside = required_element_by_id(&document.document, "inside")?;
        let after = required_element_by_id(&document.document, "after")?;
        let source_order = replay_test_source_order(
            &document,
            vec![
                SourceOrderEntry::Script(before.clone()),
                SourceOrderEntry::Iframe { in_select: false },
                SourceOrderEntry::Script(inside.clone()),
                SourceOrderEntry::Script(after.clone()),
            ],
        );

        assert!(matches!(
            &frame.data,
            markup5ever_rcdom::NodeData::Element { name, .. }
                if name.ns.as_str() == "http://www.w3.org/1999/xhtml"
                    && name.local.as_str() == "iframe"
        ));
        assert_eq!(source_order.len(), 4);
        for (actual, expected) in source_order.iter().zip([&before, &frame, &inside, &after]) {
            assert!(Rc::ptr_eq(actual, expected));
        }
        Ok(())
    }

    #[test]
    fn html_template_iframe_remains_inert_in_source_order() -> Result<(), String> {
        let document = parse_document(RcDom::default(), Default::default())
            .one("<template><iframe id=hidden></iframe></template><iframe id=visible></iframe>");
        let visible = required_element_by_id(&document.document, "visible")?;
        let source_order = replay_test_source_order(
            &document,
            vec![
                SourceOrderEntry::Iframe { in_select: false },
                SourceOrderEntry::Iframe { in_select: false },
            ],
        );

        assert_eq!(source_order.len(), 1);
        assert!(Rc::ptr_eq(&source_order[0], &visible));
        assert!(element_by_id(&document.document, "hidden").is_some());
        Ok(())
    }
}
