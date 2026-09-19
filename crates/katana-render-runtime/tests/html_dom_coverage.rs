use std::cell::RefCell;
use std::panic::AssertUnwindSafe;

use html5ever::serialize::{SerializeOpts, TraversalScope, serialize};
use katana_render_runtime::{Node, NodeData, SerializableHandle};
use markup5ever::{Attribute, QualName, local_name, ns};

#[test]
fn public_rcdom_serializes_elements_and_rejects_document_nodes() {
    assert_element_serializes();
    assert_children_only_serializes_document();
    assert_children_only_serializes_empty_document(TraversalScope::ChildrenOnly(None));
    assert_children_only_serializes_empty_document(TraversalScope::ChildrenOnly(Some(
        QualName::new(None, ns!(html), local_name!("div")),
    )));
    assert!(std::panic::catch_unwind(AssertUnwindSafe(reject_document_node)).is_err());
}

fn assert_element_serializes() {
    let element = Node::new(NodeData::Element {
        name: QualName::new(None, ns!(html), local_name!("div")),
        attrs: RefCell::new(vec![Attribute {
            name: QualName::new(None, ns!(), local_name!("id")),
            value: "coverage".into(),
        }]),
        template_contents: RefCell::new(None),
        mathml_annotation_xml_integration_point: false,
    });
    element
        .children
        .borrow_mut()
        .push(Node::new(NodeData::Text {
            contents: RefCell::new("child".into()),
        }));
    let mut output = Vec::new();
    let result = serialize(
        &mut output,
        &SerializableHandle::from(element),
        SerializeOpts {
            traversal_scope: TraversalScope::IncludeNode,
            ..SerializeOpts::default()
        },
    );
    assert!(result.is_ok());
    assert_eq!(output, b"<div id=\"coverage\">child</div>");
}

fn assert_children_only_serializes_document() {
    let document = Node::new(NodeData::Document);
    document
        .children
        .borrow_mut()
        .push(Node::new(NodeData::Text {
            contents: RefCell::new("child".into()),
        }));
    let mut output = Vec::new();
    let result = serialize(
        &mut output,
        &SerializableHandle::from(document),
        SerializeOpts {
            traversal_scope: TraversalScope::ChildrenOnly(None),
            ..SerializeOpts::default()
        },
    );
    assert!(result.is_ok());
    assert_eq!(output, b"child");
}

fn assert_children_only_serializes_empty_document(traversal_scope: TraversalScope) {
    let mut output = Vec::new();
    let result = serialize(
        &mut output,
        &SerializableHandle::from(Node::new(NodeData::Document)),
        SerializeOpts {
            traversal_scope,
            ..SerializeOpts::default()
        },
    );
    assert!(result.is_ok());
    assert!(output.is_empty());
}

fn reject_document_node() {
    let mut output = Vec::new();
    let _ = serialize(
        &mut output,
        &SerializableHandle::from(Node::new(NodeData::Document)),
        SerializeOpts {
            traversal_scope: TraversalScope::IncludeNode,
            ..SerializeOpts::default()
        },
    );
}
