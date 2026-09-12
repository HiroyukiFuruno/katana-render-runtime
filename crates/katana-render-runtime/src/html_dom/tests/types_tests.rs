use markup5ever::interface::tree_builder::TreeSink;
use markup5ever::{Attribute, QualName, local_name, ns};
use tendril::StrTendril;

use super::super::tree::append_child;
use super::super::{Node, NodeData, RcDom};
use super::{assert_panics, element, text};

#[test]
fn selectedcontent_clones_selected_option_subtree() {
    let dom = RcDom::default();
    let select = element("select");
    let selectedcontent = element("selectedcontent");
    let option = element("option");
    let option_text = text("chosen");
    if let NodeData::Element { attrs, .. } = &option.data {
        attrs.borrow_mut().push(Attribute {
            name: QualName::new(None, ns!(), local_name!("selected")),
            value: StrTendril::new(),
        });
    }
    append_child(&dom.document, select.clone());
    append_child(&select, text("ignored"));
    append_child(&select, selectedcontent.clone());
    append_child(&select, option.clone());
    append_child(&option, option_text);
    let nested = element("span");
    append_child(&option, nested.clone());
    append_child(&nested, text("nested"));
    assert!(select.selectedcontent().is_some());

    dom.maybe_clone_an_option_into_selectedcontent(&option);

    assert_eq!(selectedcontent.children.borrow().len(), 2);
    assert!(!std::rc::Rc::ptr_eq(
        &selectedcontent.children.borrow()[0],
        &option.children.borrow()[0]
    ));
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
