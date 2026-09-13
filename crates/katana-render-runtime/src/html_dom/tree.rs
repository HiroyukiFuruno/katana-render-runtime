use std::rc::Rc;

use markup5ever::interface::tree_builder::NodeOrText;

use super::{Handle, Node, NodeData};

pub(super) fn append_child(parent: &Handle, child: Handle) {
    let previous_parent = child.parent.replace(Some(Rc::downgrade(parent)));
    assert!(previous_parent.is_none());
    parent.children.borrow_mut().push(child);
}

pub(super) fn parent_and_index(target: &Handle) -> Option<(Handle, usize)> {
    let weak = target.parent.take()?;
    let Some(parent) = weak.upgrade() else {
        unreachable!("the parent weak handle must remain live")
    };
    target.parent.set(Some(weak));
    let Some((index, _)) = parent
        .children
        .borrow()
        .iter()
        .enumerate()
        .find(|(_, child)| Rc::ptr_eq(child, target))
    else {
        unreachable!("a parent must own a child that references it")
    };
    Some((parent, index))
}

pub(super) fn append_text(previous: &Handle, text: &str) -> bool {
    if let NodeData::Text { contents } = &previous.data {
        contents.borrow_mut().push_slice(text);
        true
    } else {
        false
    }
}

pub(super) fn remove_from_parent(target: &Handle) {
    if let Some((parent, index)) = parent_and_index(target) {
        parent.children.borrow_mut().remove(index);
        target.parent.set(None);
    }
}

pub(super) fn text_node(text: tendril::StrTendril) -> Handle {
    Node::new(NodeData::Text {
        contents: std::cell::RefCell::new(text),
    })
}

pub(super) fn append_before_sibling(sibling: &Handle, child: NodeOrText<Handle>) {
    let Some((parent, index)) = parent_and_index(sibling) else {
        unreachable!("a sibling must have a parent")
    };
    let child = match (child, index) {
        (NodeOrText::AppendText(text), 0) => text_node(text),
        (NodeOrText::AppendText(text), index) => {
            let children = parent.children.borrow();
            let previous = &children[index - 1];
            if append_text(previous, &text) {
                return;
            }
            text_node(text)
        }
        (NodeOrText::AppendNode(node), _) => node,
    };
    remove_from_parent(&child);
    child.parent.set(Some(Rc::downgrade(&parent)));
    parent.children.borrow_mut().insert(index, child);
}

pub(super) fn reparent_children(node: &Handle, new_parent: &Handle) {
    let mut children = node.children.borrow_mut();
    let mut new_children = new_parent.children.borrow_mut();
    for child in children.iter() {
        let previous_parent = child.parent.replace(Some(Rc::downgrade(new_parent)));
        let Some(previous_parent) = previous_parent else {
            unreachable!("a reparented child must have its previous parent")
        };
        let Some(previous_parent) = previous_parent.upgrade() else {
            unreachable!("the previous parent must remain live while reparenting")
        };
        assert!(Rc::ptr_eq(node, &previous_parent));
    }
    new_children.extend(std::mem::take(&mut *children));
}
