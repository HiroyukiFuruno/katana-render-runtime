use std::rc::Rc;

use markup5ever::interface::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use markup5ever::{Attribute, QualName, local_name, ns};

use super::super::tree::{
    append_before_sibling, append_child, append_text, parent_and_index, reparent_children,
};
use super::super::{Node, NodeData, RcDom};
use super::{assert_panics, element, html_name, parent_of, text};

#[test]
fn tree_operations_move_merge_and_reparent_nodes() {
    let first_parent = Node::new(NodeData::Document);
    let second_parent = Node::new(NodeData::Document);
    let first = text("first");
    let second = text("second");
    let inserted = element("span");

    append_child(&first_parent, first.clone());
    append_child(&first_parent, second.clone());
    assert_eq!(parent_and_index(&second).map(|(_, index)| index), Some(1));
    assert!(append_text(&first, " + text"));
    assert!(!append_text(&inserted, "ignored"));

    append_before_sibling(&second, NodeOrText::AppendNode(inserted.clone()));
    append_before_sibling(&second, NodeOrText::AppendText(" tail".into()));
    assert_eq!(first_parent.children.borrow().len(), 4);
    assert!(matches!(
        first_parent.children.borrow()[2].data,
        NodeData::Text { .. }
    ));

    reparent_children(&first_parent, &second_parent);
    assert!(first_parent.children.borrow().is_empty());
    assert_eq!(second_parent.children.borrow().len(), 4);
    assert!(second_parent
        .children
        .borrow()
        .iter()
        .all(|child| parent_of(child).is_some_and(|parent| Rc::ptr_eq(&parent, &second_parent))));
}

#[test]
fn tree_sink_exercises_document_element_template_and_attributes() {
    let dom = RcDom::default();
    let document = dom.get_document();
    let attribute = id_attribute();
    let element = dom.create_element(
        html_name("template"),
        vec![attribute.clone()],
        template_flags(),
    );
    let sibling = dom.create_element(html_name("div"), Vec::new(), ElementFlags::default());

    populate_tree_sink(&dom, &document, &element, &sibling, attribute);
    assert_template_node(&dom, &element, &sibling);
    assert_tree_sink_reparenting(dom, &document, &sibling);
}

fn id_attribute() -> Attribute {
    Attribute {
        name: QualName::new(None, ns!(), local_name!("id")),
        value: "primary".into(),
    }
}

fn template_flags() -> ElementFlags {
    let mut flags = ElementFlags::default();
    flags.template = true;
    flags.mathml_annotation_xml_integration_point = true;
    flags
}

fn populate_tree_sink(
    dom: &RcDom,
    document: &super::super::Handle,
    element: &super::super::Handle,
    sibling: &super::super::Handle,
    attribute: Attribute,
) {
    dom.append(document, NodeOrText::AppendNode(element.clone()));
    dom.append(document, NodeOrText::AppendNode(sibling.clone()));
    dom.append(sibling, NodeOrText::AppendText("one".into()));
    dom.append(sibling, NodeOrText::AppendText(" two".into()));
    dom.append_before_sibling(sibling, NodeOrText::AppendText("before".into()));
    dom.append_based_on_parent_node(sibling, element, NodeOrText::AppendText("child".into()));
    dom.add_attrs_if_missing(
        element,
        vec![
            attribute,
            Attribute {
                name: QualName::new(None, ns!(), local_name!("class")),
                value: "added".into(),
            },
        ],
    );
    append_document_metadata(dom, document);
}

fn append_document_metadata(dom: &RcDom, document: &super::super::Handle) {
    dom.append_doctype_to_document("html".into(), "public".into(), "system".into());
    dom.append(
        document,
        NodeOrText::AppendNode(dom.create_comment("note".into())),
    );
    dom.append(
        document,
        NodeOrText::AppendNode(dom.create_pi("target".into(), "data".into())),
    );
    dom.set_quirks_mode(QuirksMode::Quirks);
}

fn assert_template_node(
    dom: &RcDom,
    element: &super::super::Handle,
    sibling: &super::super::Handle,
) {
    assert!(dom.same_node(element, element));
    assert!(!dom.same_node(element, sibling));
    assert_eq!(dom.elem_name(element).local, "template");
    assert!(matches!(
        dom.get_template_contents(element).data,
        NodeData::Document
    ));
    assert!(dom.is_mathml_annotation_xml_integration_point(element));
    assert_eq!(dom.quirks_mode.get(), QuirksMode::Quirks);
    assert_eq!(
        match &element.data {
            NodeData::Element { attrs, .. } => attrs.borrow().len(),
            _ => 0,
        },
        2
    );
}

fn assert_tree_sink_reparenting(
    dom: RcDom,
    document: &super::super::Handle,
    sibling: &super::super::Handle,
) {
    dom.remove_from_parent(sibling);
    assert!(parent_of(sibling).is_none());
    dom.reparent_children(document, sibling);
    assert!(document.children.borrow().is_empty());
    assert!(!sibling.children.borrow().is_empty());
    assert_eq!(dom.finish().errors.borrow().len(), 0);
}

#[test]
fn malformed_tree_invariants_fail_closed() {
    let detached = element("div");
    assert_panics(|| append_before_sibling(&detached, NodeOrText::AppendText("x".into())));
    assert_invalid_parent_links_fail_closed();
    assert_invalid_reparenting_fails_closed();

    let parent = element("div");
    let sibling = element("span");
    append_child(&parent, sibling.clone());
    append_before_sibling(&sibling, NodeOrText::AppendText("first".into()));
}

fn assert_invalid_parent_links_fail_closed() {
    let orphan = element("span");
    let expired_parent = element("div");
    orphan.parent.set(Some(Rc::downgrade(&expired_parent)));
    drop(expired_parent);
    assert_panics(|| {
        parent_and_index(&orphan);
    });

    let missing_child = element("span");
    let parent = element("div");
    missing_child.parent.set(Some(Rc::downgrade(&parent)));
    assert_panics(|| {
        parent_and_index(&missing_child);
    });
}

fn assert_invalid_reparenting_fails_closed() {
    let source = element("div");
    source.children.borrow_mut().push(element("span"));
    assert_panics(|| reparent_children(&source, &element("section")));

    let source = element("div");
    let child = element("span");
    let expired_parent = element("article");
    child.parent.set(Some(Rc::downgrade(&expired_parent)));
    source.children.borrow_mut().push(child);
    drop(expired_parent);
    assert_panics(|| reparent_children(&source, &element("section")));
}

#[test]
fn tree_sink_rejects_invalid_handle_kinds_and_covers_detached_insertions() {
    let dom = RcDom::default();
    let document = dom.get_document();
    let previous = element("div");
    let detached = element("span");
    dom.append(&document, NodeOrText::AppendNode(previous.clone()));
    dom.append_based_on_parent_node(
        &detached,
        &previous,
        NodeOrText::AppendText("fallback".into()),
    );
    assert_eq!(previous.children.borrow().len(), 1);
    dom.create_element(html_name("div"), Vec::new(), ElementFlags::default());

    assert_non_template_handles_panic(&dom, &document);
    assert_non_element_handles_panic(&dom, &document);
}

fn assert_non_template_handles_panic(dom: &RcDom, document: &super::super::Handle) {
    assert_panics(|| {
        dom.get_template_contents(document);
    });
    let plain_element = element("div");
    assert_panics(|| {
        dom.get_template_contents(&plain_element);
    });
}

fn assert_non_element_handles_panic(dom: &RcDom, document: &super::super::Handle) {
    assert_panics(|| {
        dom.elem_name(document);
    });
    assert_panics(|| {
        dom.add_attrs_if_missing(document, Vec::new());
    });
    assert_panics(|| {
        dom.is_mathml_annotation_xml_integration_point(document);
    });
    assert_panics(|| {
        dom.maybe_clone_an_option_into_selectedcontent(document);
    });
}
