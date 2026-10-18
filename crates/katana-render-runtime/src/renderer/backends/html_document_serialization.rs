use html5ever::serialize::{SerializeOpts, TraversalScope, serialize};
use html5ever::{driver::parse_fragment_for_element, tendril::TendrilSink};
use markup5ever_rcdom::{Node, NodeData, RcDom, SerializableHandle};

pub(super) fn serialize_node(
    node: &markup5ever_rcdom::Handle,
    traversal_scope: TraversalScope,
) -> Result<String, String> {
    let mut output = Vec::new();
    serialize(
        &mut output,
        &SerializableHandle::from(node.clone()),
        SerializeOpts {
            traversal_scope,
            ..SerializeOpts::default()
        },
    )
    .map_err(serialization_error)?;
    String::from_utf8(output).map_err(utf8_error)
}

pub(super) fn serialization_error(error: std::io::Error) -> String {
    format!("HTML serialization failed: {error}")
}

pub(super) fn utf8_error(error: std::string::FromUtf8Error) -> String {
    format!("HTML serialization was not UTF-8: {error}")
}

pub(super) fn fragment_children(
    context: &markup5ever_rcdom::Handle,
    value: &str,
    error_context: &str,
) -> Result<Vec<markup5ever_rcdom::Handle>, String> {
    let (name, attributes) = match &context.data {
        NodeData::Element { name, attrs, .. } => (name.clone(), attrs.borrow().clone()),
        _ => return Err(format!("{error_context} is not an element")),
    };
    let fragment = parse_fragment_for_element(
        RcDom::default(),
        Default::default(),
        fragment_context(name, attributes),
        false,
        None,
    )
    .one(value.to_string());
    let root = super::html_document_fragment::fragment_root(&fragment.document);
    Ok(std::mem::take(&mut *root.children.borrow_mut()))
}

fn fragment_context(
    name: html5ever::QualName,
    attributes: Vec<html5ever::Attribute>,
) -> markup5ever_rcdom::Handle {
    Node::new(NodeData::Element {
        name,
        attrs: std::cell::RefCell::new(attributes),
        template_contents: Default::default(),
        mathml_annotation_xml_integration_point: false,
    })
}
