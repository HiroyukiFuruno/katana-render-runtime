/* WHY: html5ever 0.40 の未公開RcDomを、ServoのMIT OR Apache-2.0実装に基づき内部化する。 */

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fmt;
use std::mem;
use std::rc::{Rc, Weak};

use markup5ever::interface::tree_builder::{self, QuirksMode};
use markup5ever::{Attribute, QualName};
use tendril::StrTendril;

#[derive(Debug, Clone)]
pub enum NodeData {
    Document,
    Doctype {
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    },
    Text {
        contents: RefCell<StrTendril>,
    },
    Comment {
        contents: StrTendril,
    },
    Element {
        name: QualName,
        attrs: RefCell<Vec<Attribute>>,
        template_contents: RefCell<Option<Handle>>,
        mathml_annotation_xml_integration_point: bool,
    },
    ProcessingInstruction {
        target: StrTendril,
        contents: StrTendril,
    },
}

pub struct Node {
    pub parent: Cell<Option<WeakHandle>>,
    pub children: RefCell<Vec<Handle>>,
    pub data: NodeData,
}

impl Node {
    pub fn new(data: NodeData) -> Rc<Self> {
        Rc::new(Self {
            data,
            parent: Cell::new(None),
            children: RefCell::new(Vec::new()),
        })
    }

    pub(super) fn selectedcontent_ancestor(&self) -> Option<Rc<Self>> {
        let mut saw_optgroup = false;
        let mut current = self.parent().and_then(|parent| parent.upgrade())?;
        loop {
            if let NodeData::Element { name, .. } = &current.data {
                if matches!(name.local.as_str(), "datalist" | "hr" | "option") {
                    return None;
                }
                if name.local.as_str() == "optgroup" {
                    if saw_optgroup {
                        return None;
                    }
                    saw_optgroup = true;
                }
                if name.local.as_str() == "select" {
                    return Some(current);
                }
            }
            let Some(next) = current.parent().and_then(|parent| parent.upgrade()) else {
                break;
            };
            current = next;
        }
        None
    }

    pub(super) fn selectedcontent(&self) -> Option<Rc<Self>> {
        let NodeData::Element { name, attrs, .. } = &self.data else {
            unreachable!("selectedcontent is queried only for an element")
        };
        debug_assert_eq!(name.local.as_str(), "select");
        if attrs
            .borrow()
            .iter()
            .any(|attribute| attribute.name.local.as_str() == "multiple")
        {
            return None;
        }
        let mut remaining = VecDeque::from_iter(self.children.borrow().iter().cloned());
        while let Some(node) = remaining.pop_front() {
            remaining.extend(node.children.borrow().iter().cloned());
            if matches!(&node.data, NodeData::Element { name, .. } if name.local.as_str() == "selectedcontent")
            {
                return Some(node);
            }
        }
        None
    }

    pub(super) fn clone_into_selectedcontent(&self, selectedcontent: Rc<Self>) {
        *selectedcontent.children.borrow_mut() = self
            .children
            .borrow()
            .iter()
            .map(|child| child.clone_with_subtree(&selectedcontent))
            .collect();
    }

    fn clone_with_subtree(&self, parent: &Rc<Self>) -> Rc<Self> {
        let clone = Self::new(self.data.clone());
        clone.parent.set(Some(Rc::downgrade(parent)));
        *clone.children.borrow_mut() = self
            .children
            .borrow()
            .iter()
            .map(|child| child.clone_with_subtree(&clone))
            .collect();
        clone
    }

    fn parent(&self) -> Option<Weak<Self>> {
        let parent = self.parent.take();
        self.parent.set(parent.clone());
        parent
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let mut nodes = mem::take(&mut *self.children.borrow_mut());
        while let Some(node) = nodes.pop() {
            nodes.extend(mem::take(&mut *node.children.borrow_mut()));
            if let NodeData::Element {
                ref template_contents,
                ..
            } = node.data
                && let Some(template_contents) = template_contents.borrow_mut().take()
            {
                nodes.push(template_contents);
            }
        }
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("Node")
            .field("data", &self.data)
            .field("children", &self.children)
            .finish()
    }
}

pub type Handle = Rc<Node>;
pub type WeakHandle = Weak<Node>;

pub struct RcDom {
    pub document: Handle,
    pub errors: RefCell<Vec<Cow<'static, str>>>,
    pub quirks_mode: Cell<QuirksMode>,
}

impl Default for RcDom {
    fn default() -> Self {
        Self {
            document: Node::new(NodeData::Document),
            errors: Default::default(),
            quirks_mode: Cell::new(tree_builder::NoQuirks),
        }
    }
}
