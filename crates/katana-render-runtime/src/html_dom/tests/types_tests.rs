use markup5ever::interface::tree_builder::TreeSink;
use markup5ever::{Attribute, QualName, local_name, ns};
use tendril::StrTendril;

use super::super::tree::append_child;
use super::super::{Node, NodeData, RcDom};
use super::{assert_panics, element, parent_of, text};

struct RetainedTemplateFixture {
    parent: Option<super::super::Handle>,
    child: super::super::Handle,
    child_descendant: super::super::Handle,
    contents: super::super::Handle,
    contents_descendant: super::super::Handle,
}

#[test]
fn dropping_parent_preserves_retained_child_and_template_subtrees() {
    let mut fixture = retained_template_fixture();
    drop(fixture.parent.take());
    assert_retained_template_subtrees(fixture);
}

fn retained_template_fixture() -> RetainedTemplateFixture {
    let parent = Node::new(NodeData::Document);
    let child = element("template");
    let child_descendant = text("retained child");
    let contents = Node::new(NodeData::Document);
    let contents_descendant = text("retained template child");
    append_child(&child, child_descendant.clone());
    append_child(&contents, contents_descendant.clone());
    if let NodeData::Element {
        template_contents, ..
    } = &child.data
    {
        *template_contents.borrow_mut() = Some(contents.clone());
    }
    append_child(&parent, child.clone());
    RetainedTemplateFixture {
        parent: Some(parent),
        child,
        child_descendant,
        contents,
        contents_descendant,
    }
}

fn assert_retained_template_subtrees(fixture: RetainedTemplateFixture) {
    assert!(std::rc::Rc::ptr_eq(
        &fixture.child.children.borrow()[0],
        &fixture.child_descendant
    ));
    assert!(std::rc::Rc::ptr_eq(
        &fixture.contents.children.borrow()[0],
        &fixture.contents_descendant
    ));
    let NodeData::Element {
        template_contents, ..
    } = &fixture.child.data
    else {
        unreachable!("fixture creates an element")
    };
    assert!(
        template_contents
            .borrow()
            .as_ref()
            .is_some_and(|contents| std::rc::Rc::ptr_eq(contents, &fixture.contents))
    );
}

#[test]
fn dropping_unretained_template_drains_template_contents() {
    let parent = Node::new(NodeData::Document);
    let template = element("template");
    let contents = Node::new(NodeData::Document);
    let contents_weak = std::rc::Rc::downgrade(&contents);
    let NodeData::Element {
        template_contents, ..
    } = &template.data
    else {
        unreachable!("fixture creates an element")
    };
    *template_contents.borrow_mut() = Some(contents);
    append_child(&parent, template);

    drop(parent);

    assert!(contents_weak.upgrade().is_none());
}

#[test]
fn selectedcontent_clones_selected_option_subtree() {
    let dom = RcDom::default();
    let select = element("select");
    let selectedcontent = element("selectedcontent");
    let option = selected_option();
    let option_text = text("chosen");
    append_selectedcontent_fixture(&dom, &select, &selectedcontent, &option, option_text);
    assert!(select.selectedcontent().is_some());

    dom.maybe_clone_an_option_into_selectedcontent(&option);

    assert_eq!(selectedcontent.children.borrow().len(), 2);
    assert!(!std::rc::Rc::ptr_eq(
        &selectedcontent.children.borrow()[0],
        &option.children.borrow()[0]
    ));
    assert_cloned_subtree_parent_path(&dom.document, &select, &selectedcontent, &option);
}

#[test]
fn selectedcontent_clone_does_not_share_template_contents() {
    let (selectedcontent, source_template, source_contents) = selectedcontent_template_fixture();
    let cloned_template = selectedcontent.children.borrow()[0].clone();
    let cloned_contents = template_contents(&cloned_template);
    assert!(!std::rc::Rc::ptr_eq(&cloned_contents, &source_contents));
    assert!(parent_of(&cloned_contents).is_none());
    replace_text(&cloned_contents.children.borrow()[0], "changed");
    assert_text(&source_contents.children.borrow()[0], "source");
    assert_nested_template_is_independent(&source_template, &cloned_template);
}

fn selectedcontent_template_fixture() -> (
    super::super::Handle,
    super::super::Handle,
    super::super::Handle,
) {
    let dom = RcDom::default();
    let select = element("select");
    let selectedcontent = element("selectedcontent");
    let option = selected_option();
    let template = element("template");
    let source_contents = Node::new(NodeData::Document);
    append_child(&source_contents, text("source"));
    append_child(&source_contents, template_with_contents("nested"));
    set_template_contents(&template, source_contents.clone());
    append_child(&option, template.clone());
    append_selectedcontent_fixture(&dom, &select, &selectedcontent, &option, text("chosen"));
    dom.maybe_clone_an_option_into_selectedcontent(&option);
    (selectedcontent, template, source_contents)
}

fn template_with_contents(value: &str) -> super::super::Handle {
    let template = element("template");
    let contents = Node::new(NodeData::Document);
    append_child(&contents, text(value));
    set_template_contents(&template, contents);
    template
}

fn set_template_contents(template: &super::super::Handle, contents: super::super::Handle) {
    let NodeData::Element {
        template_contents, ..
    } = &template.data
    else {
        unreachable!("fixture creates a template")
    };
    *template_contents.borrow_mut() = Some(contents);
}

fn template_contents(template: &super::super::Handle) -> super::super::Handle {
    let NodeData::Element {
        template_contents, ..
    } = &template.data
    else {
        unreachable!("fixture creates a template")
    };
    let Some(contents) = template_contents.borrow().clone() else {
        unreachable!("fixture assigns template contents")
    };
    contents
}

fn assert_nested_template_is_independent(
    source_template: &super::super::Handle,
    cloned_template: &super::super::Handle,
) {
    let source_nested = template_contents(source_template).children.borrow()[1].clone();
    let cloned_nested = template_contents(cloned_template).children.borrow()[1].clone();
    assert!(!std::rc::Rc::ptr_eq(
        &template_contents(&source_nested),
        &template_contents(&cloned_nested),
    ));
}

fn replace_text(node: &super::super::Handle, value: &str) {
    let NodeData::Text { contents } = &node.data else {
        unreachable!("fixture creates text")
    };
    *contents.borrow_mut() = StrTendril::from_slice(value);
}

fn assert_text(node: &super::super::Handle, value: &str) {
    let NodeData::Text { contents } = &node.data else {
        unreachable!("fixture creates text")
    };
    assert_eq!(contents.borrow().as_ref(), value);
}

fn selected_option() -> super::super::Handle {
    let option = element("option");
    if let NodeData::Element { attrs, .. } = &option.data {
        attrs.borrow_mut().push(Attribute {
            name: QualName::new(None, ns!(), local_name!("selected")),
            value: StrTendril::new(),
        });
    }
    option
}

fn append_selectedcontent_fixture(
    dom: &RcDom,
    select: &super::super::Handle,
    selectedcontent: &super::super::Handle,
    option: &super::super::Handle,
    option_text: super::super::Handle,
) {
    append_child(&dom.document, select.clone());
    append_child(select, text("ignored"));
    append_child(select, selectedcontent.clone());
    append_child(select, option.clone());
    append_child(option, option_text);
    let nested = element("span");
    append_child(option, nested.clone());
    append_child(&nested, text("nested"));
}

fn assert_cloned_subtree_parent_path(
    document: &super::super::Handle,
    select: &super::super::Handle,
    selectedcontent: &super::super::Handle,
    source_option: &super::super::Handle,
) {
    let cloned_nested = selectedcontent.children.borrow()[1].clone();
    let path = parent_path(&cloned_nested);
    assert!(std::rc::Rc::ptr_eq(&path[1], selectedcontent));
    assert!(std::rc::Rc::ptr_eq(&path[2], select));
    assert!(std::rc::Rc::ptr_eq(&path[3], document));
    assert!(
        !path
            .iter()
            .any(|candidate| std::rc::Rc::ptr_eq(candidate, source_option))
    );
    assert_nearest_elements(&cloned_nested, selectedcontent);
}

fn parent_path(node: &super::super::Handle) -> Vec<super::super::Handle> {
    let mut path = vec![node.clone()];
    let mut current = node.clone();
    while let Some(parent) = parent_of(&current) {
        path.push(parent.clone());
        current = parent;
    }
    path
}

fn assert_nearest_elements(node: &super::super::Handle, selectedcontent: &super::super::Handle) {
    assert!(closest_element(node, "option").is_none());
    assert!(
        closest_element(node, "selectedcontent")
            .is_some_and(|candidate| std::rc::Rc::ptr_eq(&candidate, selectedcontent))
    );
}

fn closest_element(node: &super::super::Handle, local_name: &str) -> Option<super::super::Handle> {
    parent_path(node).into_iter().find(|candidate| {
        matches!(&candidate.data, NodeData::Element { name, .. } if name.local.as_str() == local_name)
    })
}

#[test]
fn selectedcontent_handles_multiple_and_invalid_ancestor_paths() {
    let select = element("select");
    let selectedcontent = element("selectedcontent");
    let option = element("option");
    append_child(&select, selectedcontent);
    append_child(&select, option.clone());
    assert!(option.selectedcontent_ancestor().is_some());
    assert_invalid_selectedcontent_ancestors(&select);
    assert_multiple_select_has_no_selectedcontent(&select);
}

fn assert_invalid_selectedcontent_ancestors(select: &super::super::Handle) {
    let blocking_parent = element("datalist");
    let blocked = element("option");
    append_child(&blocking_parent, blocked.clone());
    assert!(blocked.selectedcontent_ancestor().is_none());

    let outer_group = element("optgroup");
    let inner_group = element("optgroup");
    let grouped_option = element("option");
    append_child(select, outer_group.clone());
    append_child(&outer_group, inner_group.clone());
    append_child(&inner_group, grouped_option.clone());
    assert!(grouped_option.selectedcontent_ancestor().is_none());
}

fn assert_multiple_select_has_no_selectedcontent(select: &super::super::Handle) {
    if let NodeData::Element { attrs, .. } = &select.data {
        attrs.borrow_mut().push(Attribute {
            name: QualName::new(None, ns!(), local_name!("multiple")),
            value: StrTendril::new(),
        });
    }
    assert!(select.selectedcontent().is_none());
    assert!(element("div").selectedcontent_ancestor().is_none());
    let document = Node::new(NodeData::Document);
    let unselected = element("div");
    append_child(&document, unselected.clone());
    assert!(unselected.selectedcontent_ancestor().is_none());
    assert!(element("select").selectedcontent().is_none());
    assert_panics(|| {
        Node::new(NodeData::Document).selectedcontent();
    });
    assert!(format!("{select:?}").contains("Node"));
}
