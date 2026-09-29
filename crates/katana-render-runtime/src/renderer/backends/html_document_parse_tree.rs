use html5ever::ns;
use markup5ever_rcdom::{Handle, NodeData};

pub(super) fn document_window_load_handler_has_onload(document: &Handle) -> bool {
    let Some(html_element) = find_html_element(document) else {
        return false;
    };

    html_element
        .children
        .borrow()
        .iter()
        .any(element_has_window_load_onload)
}

fn find_html_element(document: &Handle) -> Option<Handle> {
    document.children.borrow().iter().find_map(|node| {
        matches!(&node.data, NodeData::Element { name, .. }
            if name.ns == ns!(html)
                && name.local.as_str().eq_ignore_ascii_case("html"))
        .then(|| node.clone())
    })
}

fn element_has_window_load_onload(node: &Handle) -> bool {
    let NodeData::Element { name, attrs, .. } = &node.data else {
        return false;
    };
    name.ns == ns!(html)
        && (name.local.as_str().eq_ignore_ascii_case("body")
            || name.local.as_str().eq_ignore_ascii_case("frameset"))
        && attrs.borrow().iter().any(|attribute| {
            attribute
                .name
                .local
                .to_string()
                .eq_ignore_ascii_case("onload")
        })
}

pub(super) fn visible_iframe_count(node: &Handle) -> usize {
    let is_iframe = matches!(&node.data, NodeData::Element { name, .. } if name.local.as_str().eq_ignore_ascii_case("iframe"));
    usize::from(is_iframe)
        + node
            .children
            .borrow()
            .iter()
            .map(visible_iframe_count)
            .sum::<usize>()
}
