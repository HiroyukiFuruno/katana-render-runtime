use super::RAW_TEST_SVG_ID;

pub(super) fn assert_actual_vendor_class_structure(
    raw: &str,
    labels: &[&str; 2],
) -> Result<(), String> {
    let root = xmltree::Element::parse(raw.as_bytes()).map_err(|error| error.to_string())?;
    assert_eq!(root.name, "svg");
    assert_eq!(required_attribute(&root, "id")?, RAW_TEST_SVG_ID);
    let mut ids = Vec::new();
    let mut paths = Vec::new();
    collect_raw_structure(&root, &mut ids, &mut paths)?;
    assert_class_node_ids(&ids, labels);
    assert!(!paths.is_empty());
    assert!(paths.iter().all(|path| !path.trim().is_empty()));
    Ok(())
}

fn assert_class_node_ids(ids: &[String], labels: &[&str; 2]) {
    let prefix = format!("{RAW_TEST_SVG_ID}-classId-");
    assert_eq!(ids.len(), labels.len());
    assert_eq!(
        ids.iter().collect::<std::collections::HashSet<_>>().len(),
        ids.len()
    );
    assert!(ids.iter().all(|id| id.starts_with(&prefix)));
    for label in labels {
        let label_prefix = format!("{prefix}{label}-");
        let matches: Vec<_> = ids
            .iter()
            .filter_map(|id| id.strip_prefix(&label_prefix))
            .collect();
        assert_eq!(matches.len(), 1);
        let counter = matches[0];
        assert!(!counter.is_empty() && counter.bytes().all(|byte| byte.is_ascii_digit()));
    }
}

fn required_attribute(node: &xmltree::Element, name: &str) -> Result<String, String> {
    node.attributes
        .get(name)
        .cloned()
        .ok_or_else(|| format!("missing {name} on {}", node.name))
}

fn collect_raw_structure(
    node: &xmltree::Element,
    ids: &mut Vec<String>,
    paths: &mut Vec<String>,
) -> Result<(), String> {
    if node.name == "g"
        && node
            .attributes
            .get("class")
            .is_some_and(|value| value.split_whitespace().any(|token| token == "node"))
    {
        ids.push(required_attribute(node, "id")?);
    }
    if node.name == "path" {
        paths.push(required_attribute(node, "d")?);
    }
    for child in node
        .children
        .iter()
        .filter_map(xmltree::XMLNode::as_element)
    {
        collect_raw_structure(child, ids, paths)?;
    }
    Ok(())
}

pub(super) fn assert_empty_methods_group_present(raw: &str) -> Result<(), String> {
    let root = xmltree::Element::parse(raw.as_bytes()).map_err(|error| error.to_string())?;
    assert!(contains_empty_methods_group(&root));
    Ok(())
}

fn contains_empty_methods_group(node: &xmltree::Element) -> bool {
    if node.attributes.get("class").map(String::as_str) == Some("methods-group text")
        && !node.children.iter().any(|child| match child {
            xmltree::XMLNode::Element(_) => true,
            xmltree::XMLNode::Text(text) | xmltree::XMLNode::CData(text) => !text.trim().is_empty(),
            _ => false,
        })
    {
        return true;
    }
    node.children
        .iter()
        .filter_map(xmltree::XMLNode::as_element)
        .any(contains_empty_methods_group)
}

#[test]
fn vendor_node_ids_bind_diagram_labels_and_unique_node_groups() -> Result<(), String> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" id="{RAW_TEST_SVG_ID}"><g class="node default" id="{RAW_TEST_SVG_ID}-classId-Packet-0"/><g class="default node" id="{RAW_TEST_SVG_ID}-classId-Sink-1"/><path d="M0,0L1,1"/></svg>"#
    );
    assert_actual_vendor_class_structure(&svg, &["Packet", "Sink"])?;
    let negative = [
        svg.replace(
            &format!("{RAW_TEST_SVG_ID}-classId-"),
            "other-diagram-classId-",
        ),
        svg.replace("classId-Sink-1", "classId-Unknown-1"),
        svg.replace("classId-Sink-1", "classId-Packet-0"),
        svg.replace("node default", "nonnode default"),
        svg.replace("classId-Sink-1", "classId-Sink-"),
        svg.replace("classId-Sink-1", "classId-Sink-not-a-counter"),
        svg.replace(&format!("{RAW_TEST_SVG_ID}-classId-"), "classId-"),
    ];
    for raw in negative {
        assert!(
            std::panic::catch_unwind(|| assert_actual_vendor_class_structure(
                &raw,
                &["Packet", "Sink"]
            ))
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn structure_missing_id_and_path_attributes_return_errors() {
    let without_svg_id = r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#;
    assert!(assert_actual_vendor_class_structure(without_svg_id, &["Packet", "Sink"]).is_err());
    let without_node_id = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" id="{RAW_TEST_SVG_ID}"><g class="node"/></svg>"#
    );
    assert!(assert_actual_vendor_class_structure(&without_node_id, &["Packet", "Sink"]).is_err());
    let without_path_data =
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" id="{RAW_TEST_SVG_ID}"><path/></svg>"#);
    assert!(assert_actual_vendor_class_structure(&without_path_data, &["Packet", "Sink"]).is_err());
}
