use markup5ever_rcdom::{Handle, RcDom};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub(super) enum SourceOrderEntry {
    Script(Handle),
    Iframe { in_select: bool },
}

pub(super) fn finish_source_order(
    parsed: &RcDom,
    entries: Vec<SourceOrderEntry>,
    body_onload_event_index: Option<usize>,
    body_onload_script_index: Option<usize>,
    node_visibility: &RefCell<HashMap<usize, bool>>,
) -> (Option<usize>, Vec<Handle>) {
    let mut iframes = Vec::new();
    collect_iframes(&parsed.document, &mut iframes, false, false);
    let (mut body_onload_source_order_index, source_order) = replay_source_order(
        entries,
        &iframes,
        body_onload_event_index,
        &parsed.document,
        node_visibility,
    );
    if body_onload_source_order_index.is_none() && body_onload_script_index.is_some() {
        body_onload_source_order_index = Some(source_order.len());
    }
    (body_onload_source_order_index, source_order)
}

fn replay_source_order(
    entries: Vec<SourceOrderEntry>,
    iframes: &[(Handle, bool, bool)],
    body_onload_event_index: Option<usize>,
    document: &Handle,
    node_visibility: &RefCell<HashMap<usize, bool>>,
) -> (Option<usize>, Vec<Handle>) {
    let mut iframe_index = 0;
    let mut source_order = Vec::with_capacity(entries.len());
    let mut body_onload_source_order_index = None;
    for (event_index, entry) in entries.into_iter().enumerate() {
        if body_onload_event_index == Some(event_index) {
            body_onload_source_order_index = Some(source_order.len());
        }
        match entry {
            SourceOrderEntry::Script(script) => source_order.push(script),
            SourceOrderEntry::Iframe { in_select } => append_iframe(
                &mut source_order,
                iframes,
                &mut iframe_index,
                in_select,
                document,
                node_visibility,
            ),
        }
    }
    (body_onload_source_order_index, source_order)
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
        in_template || is_element_named(node, "template"),
        in_select || is_element_named(node, "select"),
    )
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
        );

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
}
