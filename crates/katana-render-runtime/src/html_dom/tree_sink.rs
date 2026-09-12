use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use markup5ever::interface::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use markup5ever::{Attribute, ExpandedName, QualName};
use tendril::StrTendril;

use super::tree::{
    append_before_sibling, append_child, append_text, remove_from_parent, reparent_children,
    text_node,
};
use super::{Handle, Node, NodeData, RcDom};

impl TreeSink for RcDom {
    type Output = Self;
    type Handle = Handle;
    type ElemName<'a>
        = ExpandedName<'a>
    where
        Self: 'a;

    fn finish(self) -> Self {
        self
    }

    fn parse_error(&self, message: Cow<'static, str>) {
        self.errors.borrow_mut().push(message);
    }

    fn get_document(&self) -> Handle {
        self.document.clone()
    }

    fn get_template_contents(&self, target: &Handle) -> Handle {
        let NodeData::Element {
            template_contents, ..
        } = &target.data
        else {
            unreachable!("template contents are queried only for template elements")
        };
        let template_contents = template_contents.borrow();
        let Some(template_contents) = template_contents.as_ref() else {
            unreachable!("template contents are present for template elements")
        };
        template_contents.clone()
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.quirks_mode.set(mode);
    }

    fn same_node(&self, left: &Handle, right: &Handle) -> bool {
        Rc::ptr_eq(left, right)
    }

    fn elem_name<'a>(&self, target: &'a Handle) -> ExpandedName<'a> {
        let NodeData::Element { name, .. } = &target.data else {
            unreachable!("element names are queried only for elements")
        };
        name.expanded()
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        Node::new(NodeData::Element {
            name,
            attrs: RefCell::new(attrs),
            template_contents: RefCell::new(flags.template.then(|| Node::new(NodeData::Document))),
            mathml_annotation_xml_integration_point: flags.mathml_annotation_xml_integration_point,
        })
    }

    fn create_comment(&self, text: StrTendril) -> Handle {
        Node::new(NodeData::Comment { contents: text })
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Handle {
        Node::new(NodeData::ProcessingInstruction {
            target,
            contents: data,
        })
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        if let NodeOrText::AppendText(text) = &child
            && let Some(handle) = parent.children.borrow().last()
            && append_text(handle, text)
        {
            return;
        }
        append_child(
            parent,
            match child {
                NodeOrText::AppendText(text) => text_node(text),
                NodeOrText::AppendNode(node) => node,
            },
        );
    }

    fn append_before_sibling(&self, sibling: &Handle, child: NodeOrText<Handle>) {
        append_before_sibling(sibling, child);
    }

    fn append_based_on_parent_node(
        &self,
        element: &Self::Handle,
        previous_element: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        let parent = element.parent.take();
        let has_parent = parent.is_some();
        element.parent.set(parent);
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(previous_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        append_child(
            &self.document,
            Node::new(NodeData::Doctype {
                name,
                public_id,
                system_id,
            }),
        );
    }

    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        let NodeData::Element {
            attrs: existing, ..
        } = &target.data
        else {
            unreachable!("attributes are added only to elements")
        };
        let mut existing = existing.borrow_mut();
        let names = existing
            .iter()
            .map(|element| element.name.clone())
            .collect::<HashSet<_>>();
        existing.extend(
            attrs
                .into_iter()
                .filter(|attribute| !names.contains(&attribute.name)),
        );
    }

    fn remove_from_parent(&self, target: &Handle) {
        remove_from_parent(target);
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        reparent_children(node, new_parent);
    }

    fn is_mathml_annotation_xml_integration_point(&self, target: &Handle) -> bool {
        let NodeData::Element {
            mathml_annotation_xml_integration_point,
            ..
        } = target.data
        else {
            unreachable!("integration-point state is queried only for elements")
        };
        mathml_annotation_xml_integration_point
    }

    fn maybe_clone_an_option_into_selectedcontent(&self, option: &Self::Handle) {
        let NodeData::Element { name, attrs, .. } = &option.data else {
            unreachable!("selectedcontent cloning is invoked only for option elements")
        };
        debug_assert_eq!(name.local.as_str(), "option");
        if let Some(selectedcontent) = option
            .selectedcontent_ancestor()
            .and_then(|select| select.selectedcontent())
            && attrs
                .borrow()
                .iter()
                .any(|attribute| attribute.name.local.as_str() == "selected")
        {
            option.clone_into_selectedcontent(selectedcontent);
        }
    }
}
