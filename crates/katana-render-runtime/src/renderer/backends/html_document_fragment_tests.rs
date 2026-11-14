use super::test_support::{
    ADJACENT_CONTENT_CASES, ADJACENT_POSITIONS, EMPTY_HTML_MUTATION_CASES, FRAGMENT_MUTATIONS,
    FragmentMutation, assert_adjacent_content_case, run_empty_html_mutation,
};
use super::{
    HtmlDocument, Node, NodeData, fragment_context, fragment_root, insert_target, insertion_index,
    serialization_error, utf8_error,
};
use html5ever::driver::{parse_document, parse_fragment_for_element};
use html5ever::tendril::{Tendril, TendrilSink};
use markup5ever_rcdom::RcDom;
use std::rc::Rc;

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
fn empty_html_mutations_preserve_empty_content_and_tree_revision() -> Result<(), String> {
    let cases = EMPTY_HTML_MUTATION_CASES;
    for (name, mutation) in cases {
        let mut document = HtmlDocument::parse("<div id=target></div>");
        let target = document
            .get_element_by_id("target")
            .ok_or("target element was not indexed")?;
        let content_before = document.inner_html(target)?;
        let revision_before = document.tree_revision();

        run_empty_html_mutation(&mut document, target, mutation)?;

        assert_eq!(document.inner_html(target)?, content_before, "{name}");
        assert_eq!(document.tree_revision(), revision_before, "{name}");
    }
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
fn fragment_mutations_reject_missing_nodes() {
    let cases = FRAGMENT_MUTATIONS;
    for (name, mutation) in cases {
        let mut document = HtmlDocument::parse("<p id=target>Visible</p>");
        let result = match mutation {
            FragmentMutation::SetInnerHtml => document.set_inner_html(999, "<b>x</b>"),
            FragmentMutation::InsertAdjacentHtml => {
                document.insert_adjacent_html(999, "beforeend", "<b>x</b>")
            }
            FragmentMutation::InsertAdjacentText => {
                document.insert_adjacent_text(999, "beforeend", "x")
            }
        };
        assert_eq!(
            result,
            Err("HTML node 999 does not exist".to_string()),
            "{name}"
        );
    }
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

#[test]
fn insert_adjacent_content_supports_all_element_positions() -> Result<(), String> {
    for case in ADJACENT_CONTENT_CASES {
        assert_adjacent_content_case(case, &ADJACENT_POSITIONS)?;
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

    assert!(matches!(&root.data, NodeData::Element { name, .. } if name.local.as_str() == "html"));
}

#[test]
fn fragment_root_falls_back_to_document_without_body() {
    let dom = RcDom::default();
    let root = fragment_root(&dom.document);

    assert!(Rc::ptr_eq(&root, &dom.document));
}
