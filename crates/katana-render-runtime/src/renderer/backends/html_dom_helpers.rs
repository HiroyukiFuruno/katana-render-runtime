use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

pub(super) fn collect_scripts(node: &Handle, scripts: &mut Vec<String>) -> Result<(), String> {
    if let NodeData::Element { name, attrs, .. } = &node.data
        && name.local.as_str().eq_ignore_ascii_case("script")
    {
        if !script_is_executable(node) {
            return Ok(());
        }
        if let Some(source) = attribute_value(&attrs.borrow(), "src") {
            return Err(format!("external script is not supported: {source}"));
        }
        scripts.push(text_content(node));
        return Ok(());
    }
    for child in node.children.borrow().iter() {
        collect_scripts(child, scripts)?;
    }
    Ok(())
}

pub(super) fn script_is_executable(node: &Handle) -> bool {
    let NodeData::Element { attrs, .. } = &node.data else {
        return false;
    };
    let attributes = attrs.borrow();
    let kind = attribute_value(&attributes, "type").unwrap_or("");
    let kind = kind.trim_matches(|character: char| character.is_ascii_whitespace());
    kind.is_empty()
        || kind.eq_ignore_ascii_case("module")
        || matches!(
            kind.to_ascii_lowercase().as_str(),
            "application/ecmascript"
                | "application/javascript"
                | "application/x-ecmascript"
                | "application/x-javascript"
                | "text/ecmascript"
                | "text/javascript"
                | "text/javascript1.0"
                | "text/javascript1.1"
                | "text/javascript1.2"
                | "text/javascript1.3"
                | "text/javascript1.4"
                | "text/javascript1.5"
                | "text/jscript"
                | "text/livescript"
                | "text/x-ecmascript"
                | "text/x-javascript"
        )
}

pub(super) fn find_element(
    node: &Handle,
    predicate: impl Fn(&str, &[html5ever::Attribute]) -> bool + Copy,
) -> Option<Handle> {
    if let NodeData::Element { name, attrs, .. } = &node.data {
        let tag = name.local.to_string().to_ascii_lowercase();
        if predicate(&tag, &attrs.borrow()) {
            return Some(node.clone());
        }
    }
    node.children
        .borrow()
        .iter()
        .find_map(|child| find_element(child, predicate))
}

pub(super) fn attribute_value<'a>(
    attributes: &'a [html5ever::Attribute],
    name: &str,
) -> Option<&'a str> {
    attributes
        .iter()
        .find(|attribute| attribute.name.local.as_str().eq_ignore_ascii_case(name))
        .map(|attribute| attribute.value.as_ref())
}

pub(super) fn text_content(node: &Handle) -> String {
    match &node.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        _ => node.children.borrow().iter().map(text_content).collect(),
    }
}

pub(super) fn detach(node: &Handle) {
    let Some(parent) = node.parent.take().and_then(|parent| parent.upgrade()) else {
        return;
    };
    parent
        .children
        .borrow_mut()
        .retain(|child| !Rc::ptr_eq(child, node));
}

#[cfg(test)]
mod tests {
    use super::{attribute_value, detach, script_is_executable, text_content};
    use crate::renderer::backends::html_document::HtmlDocument;

    fn assert_script_type_cases(cases: &[(Option<&str>, bool)]) -> Result<(), String> {
        for (type_attribute, expected) in cases {
            let source = match type_attribute {
                Some(value) => format!(r#"<script type="{value}"></script>"#),
                None => "<script></script>".to_string(),
            };
            let mut document = HtmlDocument::parse(&source);
            let script_id = document
                .query_selector("script")
                .ok_or("script must be indexed")?;
            let script = document.node(script_id)?;
            assert_eq!(
                script_is_executable(&script),
                *expected,
                "{type_attribute:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn helper_boundaries_handle_text_nodes_and_detach_without_parent() -> Result<(), String> {
        let mut document =
            HtmlDocument::parse("<div id=target>before<span>inside</span>after</div>");
        let target_id = document
            .query_selector("#target")
            .ok_or("target must be indexed")?;
        let target = document.node(target_id)?;
        let text = target
            .children
            .borrow()
            .first()
            .cloned()
            .ok_or("text child must exist")?;

        assert!(!script_is_executable(&text));
        assert_eq!(text_content(&target), "beforeinsideafter");
        detach(&text);
        assert_eq!(text_content(&target), "insideafter");
        detach(&text);
        assert_eq!(attribute_value(&[], "missing"), None);
        Ok(())
    }

    #[test]
    fn script_type_table_distinguishes_executable_and_inert_values() -> Result<(), String> {
        assert_script_type_cases(&[
            (None, true),
            (Some(""), true),
            (Some(" \t\n "), true),
            (Some("MoDuLe"), true),
            (Some("TEXT/JAVASCRIPT"), true),
            (Some("application/json"), false),
            (Some("text/javascript; charset=utf-8"), false),
        ])
    }
}
