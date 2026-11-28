use html5ever::Attribute;
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;
use std::rc::Rc;

pub(in crate::renderer::backends) fn serialize_embedded_svg(
    node: &Handle,
    root_style: Option<&str>,
    variable_overrides: &HashMap<usize, Vec<String>>,
) -> String {
    let mut output = String::new();
    serialize_svg_node(node, &mut output, true, root_style, variable_overrides);
    output
}

fn serialize_svg_node(
    node: &Handle,
    output: &mut String,
    root: bool,
    root_style: Option<&str>,
    variable_overrides: &HashMap<usize, Vec<String>>,
) {
    match &node.data {
        NodeData::Text { contents } => output.push_str(&super::escape_xml_text(&contents.borrow())),
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.to_string();
            output.push('<');
            output.push_str(&tag);
            let pointer = Rc::as_ptr(node) as usize;
            let overrides = variable_overrides
                .get(&pointer)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            serialize_svg_attributes(&attrs.borrow(), output, root, root_style, overrides);
            output.push('>');
            for child in node.children.borrow().iter() {
                serialize_svg_node(child, output, false, None, variable_overrides);
            }
            output.push_str("</");
            output.push_str(&tag);
            output.push('>');
        }
        _ => {}
    }
}

fn serialize_svg_attributes(
    attributes: &[Attribute],
    output: &mut String,
    root: bool,
    root_style: Option<&str>,
    variable_overrides: &[String],
) {
    let has_xmlns = serialize_source_attributes(attributes, output, root, variable_overrides);
    if root {
        super::serialize_root_attributes(output, root_style, has_xmlns);
    }
}

fn serialize_source_attributes(
    attributes: &[Attribute],
    output: &mut String,
    root: bool,
    variable_overrides: &[String],
) -> bool {
    let mut has_xmlns = false;
    for attribute in attributes {
        has_xmlns |= attribute.name.local.as_str().eq_ignore_ascii_case("xmlns");
        serialize_source_attribute(attribute, output, root, variable_overrides);
    }
    serialize_variable_overrides(attributes, output, root, variable_overrides);
    has_xmlns
}

fn serialize_source_attribute(
    attribute: &Attribute,
    output: &mut String,
    root: bool,
    variable_overrides: &[String],
) {
    let local = attribute.name.local.as_str();
    if root && super::matches_ignore_ascii_case(local, &["x", "y", "width", "height", "style"]) {
        return;
    }
    if !root && local.eq_ignore_ascii_case("style") && !variable_overrides.is_empty() {
        return;
    }
    super::push_svg_attribute(
        output,
        &super::qualified_attribute_name(attribute),
        &attribute.value,
    );
}

fn serialize_variable_overrides(
    attributes: &[Attribute],
    output: &mut String,
    root: bool,
    variable_overrides: &[String],
) {
    if root || variable_overrides.is_empty() {
        return;
    }
    let style_attribute = attributes
        .iter()
        .find(|attribute| attribute.name.local.as_str().eq_ignore_ascii_case("style"))
        .map(|attribute| attribute.value.as_ref());
    let mut style = match style_attribute {
        Some(style) => style.to_owned(),
        None => String::new(),
    };
    if !style.is_empty() {
        style.push_str("; ");
    }
    style.push_str(&variable_overrides.join("; "));
    super::push_svg_attribute(output, "style", &style);
}
