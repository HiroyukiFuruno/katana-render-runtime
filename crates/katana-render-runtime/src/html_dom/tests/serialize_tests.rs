use std::io::{self, Write};

use html5ever::serialize::{Serialize, SerializeOpts, TraversalScope, serialize};
use markup5ever::interface::tree_builder::TreeSink;
use markup5ever::{Attribute, QualName, local_name, ns};
use tendril::TendrilSink;

use super::super::tree::append_child;
use super::super::{Node, NodeData, RcDom, SerializableHandle};
use super::{FailingSerializer, SerializerFailure, assert_panics, element, text};

#[test]
fn serialization_covers_include_node_and_processing_instruction() {
    let dom = RcDom::default();
    let container = element("div");
    append_serializable_children(&dom, &container);
    let output = serialize_node(container, TraversalScope::IncludeNode);
    assert!(output.is_ok());
    let Ok(output) = output else {
        return;
    };
    assert!(output.contains("<!--comment-->"));
    assert!(output.contains("<?target data>"));
    assert_html_serializer_writer_failures();
    assert_serializer_failures();
}

fn append_serializable_children(dom: &RcDom, container: &super::super::Handle) {
    append_child(&dom.document, container.clone());
    append_child(container, text("text"));
    append_child(
        container,
        Node::new(NodeData::Comment {
            contents: "comment".into(),
        }),
    );
    append_child(
        container,
        Node::new(NodeData::ProcessingInstruction {
            target: "target".into(),
            contents: "data".into(),
        }),
    );
}

fn assert_serializer_failures() {
    assert_serializer_success_paths();
    let container = element("div");
    let error = SerializableHandle::from(container).serialize(
        &mut FailingSerializer(SerializerFailure::Start),
        TraversalScope::IncludeNode,
    );
    assert!(error.is_err());
    let error = SerializableHandle::from(element("div")).serialize(
        &mut FailingSerializer(SerializerFailure::End),
        TraversalScope::IncludeNode,
    );
    assert!(error.is_err());
    assert_panics(|| {
        let _ = SerializableHandle::from(Node::new(NodeData::Document)).serialize(
            &mut FailingSerializer(SerializerFailure::Start),
            TraversalScope::IncludeNode,
        );
    });
}

fn assert_serializer_success_paths() {
    let mut serializer = FailingSerializer(SerializerFailure::Never);
    let result = SerializableHandle::from(serializable_element())
        .serialize(&mut serializer, TraversalScope::IncludeNode);
    assert!(result.is_ok());

    let result = SerializableHandle::from(document_with_child())
        .serialize(&mut serializer, TraversalScope::ChildrenOnly(None));
    assert!(result.is_ok());
}

struct CountingWriter {
    writes: usize,
    fail_after: Option<usize>,
}

impl Write for CountingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .fail_after
            .is_some_and(|allowed| self.writes >= allowed)
        {
            return Err(io::Error::other("writer failure"));
        }
        self.writes += 1;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn assert_html_serializer_writer_failures() {
    let (writes, succeeded) = serialize_with_counting_writer(None);
    assert!(succeeded);
    for fail_after in 0..writes {
        assert!(!serialize_with_counting_writer(Some(fail_after)).1);
    }
    assert!(serialize_document_with_counting_writer());
    assert_html_serializer_rejects_document();
}

fn assert_html_serializer_rejects_document() {
    assert_panics(|| {
        let mut writer = CountingWriter {
            writes: 0,
            fail_after: None,
        };
        let _ = serialize(
            &mut writer,
            &SerializableHandle::from(Node::new(NodeData::Document)),
            SerializeOpts {
                traversal_scope: TraversalScope::IncludeNode,
                ..SerializeOpts::default()
            },
        );
    });
}

fn serialize_with_counting_writer(fail_after: Option<usize>) -> (usize, bool) {
    let mut writer = CountingWriter {
        writes: 0,
        fail_after,
    };
    let result = serialize(
        &mut writer,
        &SerializableHandle::from(serializable_element()),
        SerializeOpts {
            traversal_scope: TraversalScope::IncludeNode,
            ..SerializeOpts::default()
        },
    );
    (writer.writes, result.is_ok())
}

fn serialize_document_with_counting_writer() -> bool {
    let mut writer = CountingWriter {
        writes: 0,
        fail_after: None,
    };
    serialize(
        &mut writer,
        &SerializableHandle::from(document_with_child()),
        SerializeOpts::default(),
    )
    .is_ok()
}

fn serializable_element() -> super::super::Handle {
    let node = element("div");
    if let NodeData::Element { attrs, .. } = &node.data {
        attrs.borrow_mut().push(Attribute {
            name: QualName::new(None, ns!(), local_name!("id")),
            value: "coverage".into(),
        });
    }
    append_child(&node, text("child"));
    node
}

fn document_with_child() -> super::super::Handle {
    let document = Node::new(NodeData::Document);
    append_child(&document, text("child"));
    document
}

#[test]
fn html5ever_parsing_and_serialization_cover_all_serializable_node_kinds() {
    let dom = html5ever::parse_document(RcDom::default(), Default::default()).one(
        "<!doctype html><!--comment--><html><body><p id=message>Hello</p></body></html>"
            .to_string(),
    );
    dom.parse_error("synthetic parse error".into());

    let output = serialize_node(dom.document.clone(), TraversalScope::ChildrenOnly(None));
    assert!(output.is_ok());
    let Ok(output) = output else {
        return;
    };
    assert!(output.contains("<!DOCTYPE html>"));
    assert!(output.contains("<!--comment-->"));
    assert!(output.contains("<p id=\"message\">Hello</p>"));
    assert_eq!(dom.errors.borrow().as_slice(), ["synthetic parse error"]);
}

fn serialize_node(
    node: super::super::Handle,
    traversal_scope: TraversalScope,
) -> io::Result<String> {
    let mut output = Vec::new();
    serialize(
        &mut output,
        &SerializableHandle::from(node),
        SerializeOpts {
            traversal_scope,
            ..SerializeOpts::default()
        },
    )?;
    String::from_utf8(output).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
