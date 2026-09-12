use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;

use markup5ever::serialize::TraversalScope;
use markup5ever::serialize::TraversalScope::{ChildrenOnly, IncludeNode};
use markup5ever::serialize::{AttrRef, Serialize, Serializer};
use markup5ever::{Attribute, QualName};

use super::{Handle, NodeData};

enum SerializeOp {
    Open(Handle),
    Close(QualName),
}

trait DynamicSerializer {
    fn start_elem(&mut self, name: QualName, attrs: &[AttrRef<'_>]) -> io::Result<()>;

    fn end_elem(&mut self, name: QualName) -> io::Result<()>;

    fn write_text(&mut self, text: &str) -> io::Result<()>;

    fn write_comment(&mut self, text: &str) -> io::Result<()>;

    fn write_doctype(&mut self, name: &str) -> io::Result<()>;

    fn write_processing_instruction(&mut self, target: &str, contents: &str) -> io::Result<()>;
}

struct SerializerAdapter<'a, S>(&'a mut S);

impl<S> DynamicSerializer for SerializerAdapter<'_, S>
where
    S: Serializer,
{
    fn start_elem(&mut self, name: QualName, attrs: &[AttrRef<'_>]) -> io::Result<()> {
        self.0.start_elem(name, attrs.iter().copied())
    }

    fn end_elem(&mut self, name: QualName) -> io::Result<()> {
        self.0.end_elem(name)
    }

    fn write_text(&mut self, text: &str) -> io::Result<()> {
        self.0.write_text(text)
    }

    fn write_comment(&mut self, text: &str) -> io::Result<()> {
        self.0.write_comment(text)
    }

    fn write_doctype(&mut self, name: &str) -> io::Result<()> {
        self.0.write_doctype(name)
    }

    fn write_processing_instruction(&mut self, target: &str, contents: &str) -> io::Result<()> {
        self.0.write_processing_instruction(target, contents)
    }
}

pub struct SerializableHandle(Handle);

impl SerializableHandle {
    fn new(handle: Handle) -> Self {
        Self(handle)
    }

    fn operations(&self, traversal_scope: TraversalScope) -> VecDeque<SerializeOp> {
        let mut operations = VecDeque::new();
        match traversal_scope {
            IncludeNode => operations.push_back(SerializeOp::Open(self.0.clone())),
            ChildrenOnly(_) => queue_children(&mut operations, &self.0),
        }
        operations
    }
}

impl From<Handle> for SerializableHandle {
    fn from(handle: Handle) -> Self {
        Self::new(handle)
    }
}

fn serialize_element(
    serializer: &mut dyn DynamicSerializer,
    operations: &mut VecDeque<SerializeOp>,
    handle: &Handle,
    name: &QualName,
    attrs: &RefCell<Vec<Attribute>>,
) -> io::Result<()> {
    let attributes = attrs.borrow();
    let mut pairs = Vec::with_capacity(attributes.len());
    for attribute in attributes.iter() {
        pairs.push((&attribute.name, &attribute.value[..]));
    }
    serializer.start_elem(name.clone(), &pairs)?;
    drop(attributes);
    operations.reserve(1 + handle.children.borrow().len());
    operations.push_front(SerializeOp::Close(name.clone()));
    for child in handle.children.borrow().iter().rev() {
        operations.push_front(SerializeOp::Open(child.clone()));
    }
    Ok(())
}

fn serialize_open(
    serializer: &mut dyn DynamicSerializer,
    operations: &mut VecDeque<SerializeOp>,
    handle: &Handle,
) -> io::Result<()> {
    match handle.data {
        NodeData::Element {
            ref name,
            ref attrs,
            ..
        } => serialize_element(serializer, operations, handle, name, attrs),
        NodeData::Doctype { ref name, .. } => serializer.write_doctype(name),
        NodeData::Text { ref contents } => serializer.write_text(&contents.borrow()),
        NodeData::Comment { ref contents } => serializer.write_comment(contents),
        NodeData::ProcessingInstruction {
            ref target,
            ref contents,
        } => serializer.write_processing_instruction(target, contents),
        NodeData::Document => unreachable!("only document children are serialized"),
    }
}

fn queue_children(operations: &mut VecDeque<SerializeOp>, handle: &Handle) {
    let children = handle.children.borrow();
    for child in children.iter() {
        operations.push_back(SerializeOp::Open(child.clone()));
    }
    drop(children);
}

fn serialize_operations(
    serializer: &mut dyn DynamicSerializer,
    mut operations: VecDeque<SerializeOp>,
) -> io::Result<()> {
    while let Some(operation) = operations.pop_front() {
        match operation {
            SerializeOp::Open(handle) => serialize_open(serializer, &mut operations, &handle)?,
            SerializeOp::Close(name) => serializer.end_elem(name)?,
        }
    }
    Ok(())
}

impl Serialize for SerializableHandle {
    fn serialize<S>(&self, serializer: &mut S, traversal_scope: TraversalScope) -> io::Result<()>
    where
        S: Serializer,
    {
        serialize_operations(
            &mut SerializerAdapter(serializer),
            self.operations(traversal_scope),
        )
    }
}
