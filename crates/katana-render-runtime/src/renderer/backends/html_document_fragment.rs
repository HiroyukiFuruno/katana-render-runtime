use super::html_document::HtmlDocument;
use super::html_document_serialization::{fragment_children, serialize_node};
#[cfg(test)]
use super::html_document_serialization::{serialization_error, utf8_error};
use super::html_dom_helpers::find_element;
use html5ever::serialize::TraversalScope;
use markup5ever_rcdom::{Node, NodeData};
use std::rc::Rc;

type InsertAdjacentContext = (markup5ever_rcdom::Handle, usize);

impl HtmlDocument {
    pub(super) fn inner_html(&self, node_id: u64) -> Result<String, String> {
        serialize_node(&self.node(node_id)?, TraversalScope::ChildrenOnly(None))
    }

    pub(super) fn outer_html(&self, node_id: u64) -> Result<String, String> {
        serialize_node(&self.node(node_id)?, TraversalScope::IncludeNode)
    }

    pub(super) fn set_inner_html(&mut self, node_id: u64, value: &str) -> Result<(), String> {
        let target = self.node(node_id)?;
        let children = fragment_children(&target, value, "innerHTML target")?;
        self.clear_select_value_override_for(node_id);
        let changed = !target.children.borrow().is_empty() || !children.is_empty();
        clear_children(&target);
        self.attach_children_at(&target, 0, children);
        if changed {
            self.advance_tree_revision();
        }
        Ok(())
    }

    pub(super) fn insert_adjacent_html(
        &mut self,
        node_id: u64,
        position: &str,
        value: &str,
    ) -> Result<(), String> {
        let target = self.node(node_id)?;
        if !matches!(&target.data, NodeData::Element { .. }) {
            return Err("insertAdjacentHTML target is not an element".to_string());
        }
        let position = position.trim().to_ascii_lowercase();
        let (container, index) = insert_target(&target, &position)?;
        let children = fragment_children(&container, value, "insertAdjacentHTML container")?;
        let changed = !children.is_empty();
        if changed && let Some(container_id) = self.node_id_for_handle(&container) {
            self.clear_select_value_override_for(container_id);
        }
        self.attach_children_at(&container, index, children);
        if changed {
            self.advance_tree_revision();
        }
        Ok(())
    }

    pub(super) fn insert_adjacent_text(
        &mut self,
        node_id: u64,
        position: &str,
        value: &str,
    ) -> Result<(), String> {
        let target = self.node(node_id)?;
        if !matches!(&target.data, NodeData::Element { .. }) {
            return Err("insertAdjacentText target is not an element".to_string());
        }
        let (container, index) = insert_target(&target, &position.trim().to_ascii_lowercase())?;
        if let Some(container_id) = self.node_id_for_handle(&container) {
            self.clear_select_value_override_for(container_id);
        }
        let text = Node::new(NodeData::Text {
            contents: std::cell::RefCell::new(value.into()),
        });
        self.attach_children_at(&container, index, vec![text]);
        self.advance_tree_revision();
        Ok(())
    }

    fn attach_children_at(
        &mut self,
        target: &markup5ever_rcdom::Handle,
        index: usize,
        children: Vec<markup5ever_rcdom::Handle>,
    ) {
        for (offset, child) in children.into_iter().enumerate() {
            child.parent.set(Some(Rc::downgrade(target)));
            target
                .children
                .borrow_mut()
                .insert(index + offset, child.clone());
            self.register_subtree(&child);
        }
    }
}

fn insert_target(
    target: &markup5ever_rcdom::Handle,
    position: &str,
) -> Result<InsertAdjacentContext, String> {
    let is_after_begin = position == "afterbegin";
    let is_before_end = position == "beforeend";
    if is_after_begin {
        return Ok((target.clone(), 0));
    }
    if is_before_end {
        return Ok((target.clone(), target.children.borrow().len()));
    }
    let is_before_begin = position == "beforebegin";
    if is_before_begin || position == "afterend" {
        let parent = parent(target)
            .ok_or_else(|| format!("insertAdjacentHTML {position} requires a parent"))?;
        if !matches!(&parent.data, NodeData::Element { .. }) {
            return Err(format!(
                "insertAdjacentHTML {position} requires an element parent"
            ));
        }
        return insertion_index(&parent, target, position == "afterend")
            .map(|index| (parent, index));
    }
    Err(format!(
        "unsupported insertAdjacentHTML position: {position}"
    ))
}

fn insertion_index(
    parent: &markup5ever_rcdom::Handle,
    target: &markup5ever_rcdom::Handle,
    after_end: bool,
) -> Result<usize, String> {
    let target_index = parent
        .children
        .borrow()
        .iter()
        .position(|child| Rc::ptr_eq(child, target))
        .ok_or_else(|| "insertAdjacentHTML target is detached".to_string())?;
    Ok(target_index + usize::from(after_end))
}

fn parent(node: &markup5ever_rcdom::Handle) -> Option<markup5ever_rcdom::Handle> {
    let parent = node.parent.take();
    node.parent.set(parent.clone());
    parent.and_then(|parent| parent.upgrade())
}

fn clear_children(target: &markup5ever_rcdom::Handle) {
    let previous = std::mem::take(&mut *target.children.borrow_mut());
    for child in previous {
        child.parent.set(None);
    }
}

#[cfg(test)]
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

pub(super) fn fragment_root(document: &markup5ever_rcdom::Handle) -> markup5ever_rcdom::Handle {
    if let Some(body) = find_element(document, is_body) {
        return body;
    }
    if let Some(html) = find_element(document, is_html) {
        return html;
    }
    document.clone()
}

fn is_body(tag: &str, _attributes: &[html5ever::Attribute]) -> bool {
    tag == "body"
}

fn is_html(tag: &str, _attributes: &[html5ever::Attribute]) -> bool {
    tag == "html"
}

#[cfg(test)]
mod tests {
    use super::HtmlDocument;
    use super::*;
    use html5ever::driver::parse_document;
    use html5ever::driver::parse_fragment_for_element;
    use html5ever::tendril::Tendril;
    use html5ever::tendril::TendrilSink;
    use markup5ever_rcdom::RcDom;

    #[test]
    fn rejects_inner_html_on_the_document_root() {
        let mut document = HtmlDocument::parse("<p id=target>Visible</p>");

        let result = document.set_inner_html(1, "<span>replacement</span>");

        assert!(matches!(result, Err(error) if error.contains("not an element")));
    }

    #[test]
    fn rejects_inner_html_on_missing_node() {
        let mut document = HtmlDocument::parse("<p id=target>Visible</p>");

        assert_eq!(
            document.set_inner_html(999, "<span>replacement</span>"),
            Err("HTML node 999 does not exist".to_string())
        );
    }

    #[test]
    fn replaces_element_inner_html_with_parsed_fragment_children() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<div id=target>Visible</div>");
        let target = document
            .get_element_by_id("target")
            .ok_or_else(|| "target element was not indexed".to_string())?;

        document
            .set_inner_html(target, "<span>replacement</span>")
            .map_err(|error| format!("unexpected test error: {error}"))?;

        assert_eq!(
            document
                .inner_html(target)
                .map_err(|error| format!("unexpected test error: {error}"))?,
            "<span>replacement</span>".to_string()
        );
        Ok(())
    }

    #[test]
    fn list_inner_html_attaches_items_without_a_document_wrapper() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<ul id=target></ul>");
        let target_id = document
            .get_element_by_id("target")
            .ok_or_else(|| "target element was not indexed".to_string())?;

        document
            .set_inner_html(target_id, "<li id=item>Task</li>")
            .map_err(|error| format!("unexpected test error: {error}"))?;

        let target = document
            .node(target_id)
            .map_err(|error| format!("unexpected test error: {error}"))?;
        let children = target.children.borrow();
        assert_eq!(children.len(), 1);
        assert!(
            matches!(
                &children[0].data,
                NodeData::Element { name, .. } if name.local.as_str() == "li"
            ),
            "fragment child was not a list item"
        );
        assert!(document.get_element_by_id("item").is_some());
        Ok(())
    }

    #[test]
    fn inner_html_rejects_missing_node() {
        let document = HtmlDocument::parse("<p id=target>Visible</p>");

        assert_eq!(
            document.inner_html(999),
            Err("HTML node 999 does not exist".to_string())
        );
    }

    #[test]
    fn serializes_outer_html_without_static_export_transforms() -> Result<(), String> {
        let mut document =
            HtmlDocument::parse("<main><aside id=target class=learn><b>Info</b></aside></main>");
        let target = document
            .get_element_by_id("target")
            .ok_or_else(|| "target must exist".to_string())?;

        assert_eq!(
            document
                .outer_html(target)
                .map_err(|error| format!("unexpected test error: {error}"))?,
            "<aside id=\"target\" class=\"learn\"><b>Info</b></aside>"
        );
        assert_eq!(
            document
                .inner_html(target)
                .map_err(|error| format!("unexpected test error: {error}"))?,
            "<b>Info</b>"
        );
        Ok(())
    }

    type AdjacentInsert = fn(&mut HtmlDocument, u64, &str, &str) -> Result<(), String>;

    struct AdjacentContentCase {
        kind: &'static str,
        insert: AdjacentInsert,
        values: [&'static str; 4],
        expected: &'static str,
    }

    const ADJACENT_POSITIONS: [&str; 4] = ["beforebegin", "afterbegin", "beforeend", "afterend"];
    const ADJACENT_CONTENT_CASES: [AdjacentContentCase; 2] = [
        AdjacentContentCase {
            kind: "HTML",
            insert: HtmlDocument::insert_adjacent_html,
            values: [
                "<i>before</i>",
                "<b>first</b>",
                "<b>last</b>",
                "<i>after</i>",
            ],
            expected: "<i>before</i><p id=\"target\"><b>first</b>Target<b>last</b></p><i>after</i>",
        },
        AdjacentContentCase {
            kind: "Text",
            insert: HtmlDocument::insert_adjacent_text,
            values: [
                "<outer-before>",
                "<inner-first>",
                "<inner-last>",
                "<outer-after>",
            ],
            expected: "&lt;outer-before&gt;<p id=\"target\">&lt;inner-first&gt;Target&lt;inner-last&gt;</p>&lt;outer-after&gt;",
        },
    ];

    #[test]
    fn insert_adjacent_content_supports_all_element_positions() -> Result<(), String> {
        for case in ADJACENT_CONTENT_CASES {
            assert_adjacent_content_case(case, &ADJACENT_POSITIONS)?;
        }
        Ok(())
    }

    fn assert_adjacent_content_case(
        case: AdjacentContentCase,
        positions: &[&str; 4],
    ) -> Result<(), String> {
        let mut document = HtmlDocument::parse("<main id=parent><p id=target>Target</p></main>");
        let target = document
            .get_element_by_id("target")
            .ok_or_else(|| "target must exist".to_string())?;
        let parent = document
            .get_element_by_id("parent")
            .ok_or_else(|| "parent must exist".to_string())?;
        for (position, value) in positions.iter().zip(case.values) {
            (case.insert)(&mut document, target, position, value)?;
        }
        assert_eq!(document.inner_html(parent)?, case.expected, "{}", case.kind);
        if case.kind == "Text" {
            assert!(matches!(
                document.insert_adjacent_text(1, "beforeend", "ignored"),
                Err(error) if error == "insertAdjacentText target is not an element"
            ));
        }
        Ok(())
    }

    #[test]
    fn insert_adjacent_html_rejects_invalid_targets_positions_and_parents() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<p id=target>Target</p>");
        let target = document
            .get_element_by_id("target")
            .ok_or_else(|| "target must exist".to_string())?;

        assert!(matches!(
            document.insert_adjacent_html(1, "beforeend", "<b>x</b>"),
            Err(error) if error == "insertAdjacentHTML target is not an element"
        ));
        assert!(matches!(
            document.insert_adjacent_html(target, "middle", "<b>x</b>"),
            Err(error) if error == "unsupported insertAdjacentHTML position: middle"
        ));

        let detached = document
            .create_element("aside")
            .map_err(|error| format!("unexpected test error: {error}"))?;
        assert!(matches!(
            document.insert_adjacent_html(detached, "beforebegin", "<b>x</b>"),
            Err(error) if error == "insertAdjacentHTML beforebegin requires a parent"
        ));
        Ok(())
    }

    #[test]
    fn insert_target_reports_adjacent_position_indexes() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<ul id=target><li>seed</li></ul>");
        let target = document
            .get_element_by_id("target")
            .ok_or_else(|| "target must exist".to_string())?;
        let target_node = document.node(target)?;

        let (_, after_begin_index) = insert_target(&target_node, "afterbegin")?;
        let (_, before_end_index) = insert_target(&target_node, "beforeend")?;

        assert_eq!(after_begin_index, 0);
        assert_eq!(before_end_index, 1);
        Ok(())
    }

    #[test]
    fn insert_target_rejects_non_element_parent() {
        let parent = Node::new(NodeData::Text {
            contents: std::cell::RefCell::new(Tendril::from("parent")),
        });
        let target = Node::new(NodeData::Element {
            name: html5ever::QualName::new(None, html5ever::ns!(html), "span".into()),
            attrs: std::cell::RefCell::new(Vec::new()),
            template_contents: Default::default(),
            mathml_annotation_xml_integration_point: false,
        });
        target.parent.set(Some(std::rc::Rc::downgrade(&parent)));
        parent.children.borrow_mut().push(target.clone());

        let result = insert_target(&target, "beforebegin");

        assert_eq!(
            result.err().as_deref(),
            Some("insertAdjacentHTML beforebegin requires an element parent")
        );
    }

    #[test]
    fn insertion_index_rejects_a_detached_target() {
        let parent = fragment_context(
            html5ever::QualName::new(None, html5ever::ns!(html), "main".into()),
            Vec::new(),
        );
        let target = fragment_context(
            html5ever::QualName::new(None, html5ever::ns!(html), "span".into()),
            Vec::new(),
        );

        assert_eq!(
            insertion_index(&parent, &target, false),
            Err("insertAdjacentHTML target is detached".to_string())
        );
    }

    #[test]
    fn serialization_error_helpers_preserve_context() {
        assert_eq!(
            serialization_error(std::io::Error::other("writer failed")),
            "HTML serialization failed: writer failed"
        );
        let invalid = String::from_utf8(vec![0xff]);
        assert!(invalid.is_err());
        let mut errors = invalid.err().into_iter().collect::<Vec<_>>();
        assert!(
            utf8_error(errors.remove(0))
                .starts_with("HTML serialization was not UTF-8: invalid utf-8 sequence")
        );
    }

    #[test]
    fn fragment_root_prefers_body_when_present() {
        let dom = parse_document(RcDom::default(), Default::default())
            .one("<html><body><span>body</span></body></html>");
        let root = fragment_root(&dom.document);

        assert!(
            matches!(&root.data, markup5ever_rcdom::NodeData::Element { name, .. } if name.local.as_str() == "body")
        );
    }

    #[test]
    fn fragment_root_uses_html_as_the_fragment_container_without_a_body() {
        let dom = parse_fragment_for_element(
            RcDom::default(),
            Default::default(),
            fragment_context(
                html5ever::QualName::new(None, html5ever::ns!(html), html5ever::local_name!("ul")),
                Vec::new(),
            ),
            false,
            None,
        )
        .one("<li>item</li>");
        let root = fragment_root(&dom.document);

        assert!(
            matches!(&root.data, NodeData::Element { name, .. } if name.local.as_str() == "html")
        );
    }

    #[test]
    fn fragment_root_falls_back_to_document_without_body() {
        let dom = RcDom::default();
        let root = fragment_root(&dom.document);

        assert!(Rc::ptr_eq(&root, &dom.document));
    }
}
