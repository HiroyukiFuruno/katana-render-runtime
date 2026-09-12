use markup5ever::interface::tree_builder::TreeSink;
use markup5ever::{Attribute, QualName, local_name, ns};
use tendril::StrTendril;

use super::super::tree::append_child;
use super::super::{Node, NodeData, RcDom};
use super::{assert_panics, element, parent_of, text};

#[test]
fn selectedcontent_clones_selected_option_subtree() {
    let dom = RcDom::default();
    let select = element("select");
    let selectedcontent = element("selectedcontent");
    let option = selected_option();
    let option_text = text("chosen");
    append_selectedcontent_fixture(&dom, &select, &selectedcontent, &option, option_text);
    assert!(select.selectedcontent().is_some());

    dom.maybe_clone_an_option_into_selectedcontent(&option);

    assert_eq!(selectedcontent.children.borrow().len(), 2);
    assert!(!std::rc::Rc::ptr_eq(
        &selectedcontent.children.borrow()[0],
        &option.children.borrow()[0]
    ));
    assert_cloned_subtree_parent_path(&dom.document, &select, &selectedcontent, &option);
}

fn selected_option() -> super::super::Handle {
    let option = element("option");
    if let NodeData::Element { attrs, .. } = &option.data {
        attrs.borrow_mut().push(Attribute {
            name: QualName::new(None, ns!(), local_name!("selected")),
            value: StrTendril::new(),
        });
    }
    option
}

fn append_selectedcontent_fixture(
    dom: &RcDom,
    select: &super::super::Handle,
    selectedcontent: &super::super::Handle,
    option: &super::super::Handle,
    option_text: super::super::Handle,
) {
    append_child(&dom.document, select.clone());
    append_child(select, text("ignored"));
    append_child(select, selectedcontent.clone());
    append_child(select, option.clone());
    append_child(option, option_text);
    let nested = element("span");
    append_child(option, nested.clone());
    append_child(&nested, text("nested"));
}

fn assert_cloned_subtree_parent_path(
    document: &super::super::Handle,
    select: &super::super::Handle,
    selectedcontent: &super::super::Handle,
    source_option: &super::super::Handle,
) {
    let cloned_nested = selectedcontent.children.borrow()[1].clone();
    let path = parent_path(&cloned_nested);
    assert!(std::rc::Rc::ptr_eq(&path[1], selectedcontent));
    assert!(std::rc::Rc::ptr_eq(&path[2], select));
    assert!(std::rc::Rc::ptr_eq(&path[3], document));
    assert!(
        !path
            .iter()
            .any(|candidate| std::rc::Rc::ptr_eq(candidate, source_option))
    );
    assert_nearest_elements(&cloned_nested, selectedcontent);
}

fn parent_path(node: &super::super::Handle) -> Vec<super::super::Handle> {
    let mut path = vec![node.clone()];
    let mut current = node.clone();
    while let Some(parent) = parent_of(&current) {
        path.push(parent.clone());
        current = parent;
    }
    path
}

fn assert_nearest_elements(node: &super::super::Handle, selectedcontent: &super::super::Handle) {
    assert!(closest_element(node, "option").is_none());
    assert!(
        closest_element(node, "selectedcontent")
            .is_some_and(|candidate| std::rc::Rc::ptr_eq(&candidate, selectedcontent))
    );
}

fn closest_element(node: &super::super::Handle, local_name: &str) -> Option<super::super::Handle> {
    parent_path(node).into_iter().find(|candidate| {
        matches!(&candidate.data, NodeData::Element { name, .. } if name.local.as_str() == local_name)
    })
}

#[test]
fn selectedcontent_handles_multiple_and_invalid_ancestor_paths() {
    let select = element("select");
    let selectedcontent = element("selectedcontent");
    let option = element("option");
    append_child(&select, selectedcontent);
    append_child(&select, option.clone());
    assert!(option.selectedcontent_ancestor().is_some());
    assert_invalid_selectedcontent_ancestors(&select);
    assert_multiple_select_has_no_selectedcontent(&select);
}

fn assert_invalid_selectedcontent_ancestors(select: &super::super::Handle) {
    let blocking_parent = element("datalist");
    let blocked = element("option");
    append_child(&blocking_parent, blocked.clone());
    assert!(blocked.selectedcontent_ancestor().is_none());

    let outer_group = element("optgroup");
    let inner_group = element("optgroup");
    let grouped_option = element("option");
    append_child(select, outer_group.clone());
    append_child(&outer_group, inner_group.clone());
    append_child(&inner_group, grouped_option.clone());
    assert!(grouped_option.selectedcontent_ancestor().is_none());
}

fn assert_multiple_select_has_no_selectedcontent(select: &super::super::Handle) {
    if let NodeData::Element { attrs, .. } = &select.data {
        attrs.borrow_mut().push(Attribute {
            name: QualName::new(None, ns!(), local_name!("multiple")),
            value: StrTendril::new(),
        });
    }
    assert!(select.selectedcontent().is_none());
    assert!(element("div").selectedcontent_ancestor().is_none());
    let document = Node::new(NodeData::Document);
    let unselected = element("div");
    append_child(&document, unselected.clone());
    assert!(unselected.selectedcontent_ancestor().is_none());
    assert!(element("select").selectedcontent().is_none());
    assert_panics(|| {
        Node::new(NodeData::Document).selectedcontent();
    });
    assert!(format!("{select:?}").contains("Node"));
}
