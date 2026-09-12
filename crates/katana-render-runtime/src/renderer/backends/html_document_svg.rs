use html5ever::Attribute;
use markup5ever_rcdom::{Handle, NodeData};

pub(in crate::renderer::backends) const EMBEDDED_SVG_MARKUP_ATTRIBUTE: &str =
    "__krr_embedded_svg_markup";
pub(in crate::renderer::backends) const EMBEDDED_SVG_X_PLACEHOLDER: &str = "__KRR_SVG_X__";
pub(in crate::renderer::backends) const EMBEDDED_SVG_Y_PLACEHOLDER: &str = "__KRR_SVG_Y__";
pub(in crate::renderer::backends) const EMBEDDED_SVG_WIDTH_PLACEHOLDER: &str = "__KRR_SVG_WIDTH__";
pub(in crate::renderer::backends) const EMBEDDED_SVG_HEIGHT_PLACEHOLDER: &str =
    "__KRR_SVG_HEIGHT__";

pub(super) fn serialize_embedded_svg(node: &Handle, root_style: Option<&str>) -> String {
    let mut output = String::new();
    serialize_svg_node(node, &mut output, true, root_style);
    output
}

fn serialize_svg_node(node: &Handle, output: &mut String, root: bool, root_style: Option<&str>) {
    match &node.data {
        NodeData::Text { contents } => output.push_str(&escape_xml(&contents.borrow())),
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.to_string();
            output.push('<');
            output.push_str(&tag);
            serialize_svg_attributes(&attrs.borrow(), output, root, root_style);
            output.push('>');
            for child in node.children.borrow().iter() {
                serialize_svg_node(child, output, false, None);
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
) {
    let has_xmlns = serialize_source_attributes(attributes, output, root);
    if root {
        serialize_root_attributes(output, root_style, has_xmlns);
    }
}

fn serialize_source_attributes(attributes: &[Attribute], output: &mut String, root: bool) -> bool {
    let mut has_xmlns = false;
    for attribute in attributes {
        let local = attribute.name.local.as_str();
        has_xmlns |= local.eq_ignore_ascii_case("xmlns");
        if root && matches_ignore_ascii_case(local, &["x", "y", "width", "height", "style"]) {
            continue;
        }
        push_svg_attribute(
            output,
            &qualified_attribute_name(attribute),
            &attribute.value,
        );
    }
    has_xmlns
}

fn serialize_root_attributes(output: &mut String, root_style: Option<&str>, has_xmlns: bool) {
    if !has_xmlns {
        push_svg_attribute(output, "xmlns", "http://www.w3.org/2000/svg");
    }
    root_style
        .filter(|style| !style.is_empty())
        .iter()
        .for_each(|style| push_svg_attribute(output, "style", style));
    push_svg_attribute(output, "x", EMBEDDED_SVG_X_PLACEHOLDER);
    push_svg_attribute(output, "y", EMBEDDED_SVG_Y_PLACEHOLDER);
    push_svg_attribute(output, "width", EMBEDDED_SVG_WIDTH_PLACEHOLDER);
    push_svg_attribute(output, "height", EMBEDDED_SVG_HEIGHT_PLACEHOLDER);
}

fn matches_ignore_ascii_case(value: &str, expected: &[&str]) -> bool {
    expected
        .iter()
        .any(|candidate| value.eq_ignore_ascii_case(candidate))
}

fn qualified_attribute_name(attribute: &Attribute) -> String {
    attribute.name.prefix.as_ref().map_or_else(
        || attribute.name.local.to_string(),
        |prefix| format!("{prefix}:{}", attribute.name.local),
    )
}

fn push_svg_attribute(output: &mut String, name: &str, value: &str) {
    output.push(' ');
    output.push_str(name);
    output.push_str("=\"");
    output.push_str(&escape_xml(value));
    output.push('"');
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::{
        EMBEDDED_SVG_HEIGHT_PLACEHOLDER, EMBEDDED_SVG_WIDTH_PLACEHOLDER,
        EMBEDDED_SVG_X_PLACEHOLDER, EMBEDDED_SVG_Y_PLACEHOLDER, serialize_root_attributes,
    };

    #[test]
    fn serialize_root_attributes_adds_missing_xmlns_and_geometry_placeholders() {
        let mut output = String::new();
        serialize_root_attributes(&mut output, Some("width: 40px"), false);

        assert!(output.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(output.contains("style=\"width: 40px\""));
        assert!(output.contains(format!("x=\"{}\"", EMBEDDED_SVG_X_PLACEHOLDER).as_str()));
        assert!(output.contains(format!("y=\"{}\"", EMBEDDED_SVG_Y_PLACEHOLDER).as_str()));
        assert!(output.contains(format!("width=\"{}\"", EMBEDDED_SVG_WIDTH_PLACEHOLDER).as_str()));
        assert!(
            output.contains(format!("height=\"{}\"", EMBEDDED_SVG_HEIGHT_PLACEHOLDER).as_str())
        );
    }

    #[test]
    fn serialize_root_attributes_keeps_existing_xmlns_if_present() {
        let mut output = String::new();
        serialize_root_attributes(&mut output, None, true);

        assert!(!output.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(output.contains(format!("x=\"{}\"", EMBEDDED_SVG_X_PLACEHOLDER).as_str()));
    }
}
