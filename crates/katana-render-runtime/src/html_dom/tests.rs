use std::io;
use std::panic::AssertUnwindSafe;

use html5ever::serialize::{AttrRef, Serializer};
use markup5ever::QualName;

use super::{Handle, Node, NodeData};

pub(super) fn html_name(local: &str) -> QualName {
    QualName::new(None, markup5ever::ns!(html), local.into())
}

pub(super) fn element(name: &str) -> Handle {
    Node::new(NodeData::Element {
        name: html_name(name),
        attrs: std::cell::RefCell::new(Vec::new()),
        template_contents: std::cell::RefCell::new(None),
        mathml_annotation_xml_integration_point: false,
    })
}

pub(super) fn text(value: &str) -> Handle {
    Node::new(NodeData::Text {
        contents: std::cell::RefCell::new(value.into()),
    })
}

pub(super) fn parent_of(node: &Handle) -> Option<Handle> {
    let parent = node.parent.take();
    node.parent.set(parent.clone());
    parent.and_then(|parent| parent.upgrade())
}

pub(super) fn assert_panics(operation: impl FnOnce()) {
    assert!(std::panic::catch_unwind(AssertUnwindSafe(operation)).is_err());
}

pub(super) enum SerializerFailure {
    Never,
    Start,
    End,
}

pub(super) struct FailingSerializer(pub(super) SerializerFailure);

impl Serializer for FailingSerializer {
    fn start_elem<'a, AttrIter>(&mut self, _name: QualName, _attrs: AttrIter) -> io::Result<()>
    where
        AttrIter: Iterator<Item = AttrRef<'a>>,
    {
        if matches!(self.0, SerializerFailure::Start) {
            Err(io::Error::other("start element failure"))
        } else {
            Ok(())
        }
    }

    fn end_elem(&mut self, _name: QualName) -> io::Result<()> {
        if matches!(self.0, SerializerFailure::End) {
            Err(io::Error::other("end element failure"))
        } else {
            Ok(())
        }
    }

    fn write_text(&mut self, _text: &str) -> io::Result<()> {
        Ok(())
    }

    fn write_comment(&mut self, _text: &str) -> io::Result<()> {
        Ok(())
    }

    fn write_doctype(&mut self, _name: &str) -> io::Result<()> {
        Ok(())
    }

    fn write_processing_instruction(&mut self, _target: &str, _data: &str) -> io::Result<()> {
        Ok(())
    }
}

mod serialize_tests;
mod tree_tests;
mod types_tests;
