use markup5ever_rcdom::{Handle, RcDom};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub(super) enum SourceOrderEntry {
    Script(Handle),
    Iframe,
}

pub(super) fn finish_source_order(
    parsed: &RcDom,
    entries: Vec<SourceOrderEntry>,
    body_onload_event_index: Option<usize>,
    body_onload_script_index: Option<usize>,
    node_visibility: &RefCell<HashMap<usize, bool>>,
) -> (Option<usize>, Vec<Handle>) {
    let mut iframes = Vec::new();
    collect_iframes(&parsed.document, &mut iframes, false);
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

pub(super) fn iframe_count(document: &Handle) -> usize {
    let mut iframes = Vec::new();
    collect_iframes(document, &mut iframes, false);
    iframes.len()
}

pub(super) fn last_iframe_is_in_select(document: &Handle) -> bool {
    let mut iframes = Vec::new();
    collect_iframes(document, &mut iframes, false);
    let Some((iframe, _)) = iframes.last() else {
        return false;
    };
    let mut current = iframe.clone();
    loop {
        let parent = current.parent.take();
        current.parent.set(parent.clone());
        let Some(parent) = parent.and_then(|parent| parent.upgrade()) else {
            return false;
        };
        if matches!(&parent.data, markup5ever_rcdom::NodeData::Element { name, .. }
            if name.local.as_str().eq_ignore_ascii_case("select"))
        {
            return true;
        }
        current = parent;
    }
}

fn replay_source_order(
    entries: Vec<SourceOrderEntry>,
    iframes: &[(Handle, bool)],
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
            SourceOrderEntry::Iframe => append_iframe(
                &mut source_order,
                iframes,
                &mut iframe_index,
                document,
                node_visibility,
            ),
        }
    }
    (body_onload_source_order_index, source_order)
}

fn append_iframe(
    source_order: &mut Vec<Handle>,
    iframes: &[(Handle, bool)],
    iframe_index: &mut usize,
    document: &Handle,
    node_visibility: &RefCell<HashMap<usize, bool>>,
) {
    let Some((iframe, in_template)) = iframes.get(*iframe_index) else {
        return;
    };
    *iframe_index += 1;
    if *in_template || !node_is_visible(iframe, document, node_visibility) {
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

fn collect_iframes(node: &Handle, iframes: &mut Vec<(Handle, bool)>, in_template: bool) {
    let in_template = in_template
        || matches!(&node.data, markup5ever_rcdom::NodeData::Element { name, .. }
            if name.local.as_str().eq_ignore_ascii_case("template"));
    if matches!(&node.data, markup5ever_rcdom::NodeData::Element { name, .. }
        if name.local.as_str().eq_ignore_ascii_case("iframe"))
    {
        iframes.push((node.clone(), in_template));
    }
    let Ok(children) = node.children.try_borrow() else {
        return;
    };
    for child in children.iter() {
        collect_iframes(child, iframes, in_template);
    }
    if let markup5ever_rcdom::NodeData::Element {
        template_contents, ..
    } = &node.data
        && let Some(template_contents) = template_contents.borrow().as_ref()
    {
        collect_iframes(template_contents, iframes, true);
    }
}
