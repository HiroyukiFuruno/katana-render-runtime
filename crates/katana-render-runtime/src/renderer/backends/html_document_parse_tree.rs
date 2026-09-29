use html5ever::ns;
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

#[derive(Default)]
pub(super) struct WindowLoadHandlerObserver {
    html_element: Option<Handle>,
    body_element: Option<Handle>,
    body_lookup_completed: bool,
    frameset_element: Option<Handle>,
}

impl WindowLoadHandlerObserver {
    pub(super) fn token_created_or_updated_window_load_handler(
        &mut self,
        document: &Handle,
        token_name: &str,
    ) -> bool {
        let Some(html_element) = self.html_element(document) else {
            return false;
        };

        if token_name.eq_ignore_ascii_case("body") {
            return self.body_candidate_has_onload(&html_element);
        }

        if !token_name.eq_ignore_ascii_case("frameset") {
            return false;
        }

        self.frameset_candidate_has_onload(&html_element)
    }

    fn body_candidate_has_onload(&mut self, html_element: &Handle) -> bool {
        self.body_element(html_element)
            .is_some_and(element_has_window_load_onload)
    }

    fn frameset_candidate_has_onload(&mut self, html_element: &Handle) -> bool {
        if self.frameset_element.is_none() {
            let body = self.body_element(html_element);
            let body_remains_in_document = body.is_some_and(|body| has_parent(body, html_element));
            if body_remains_in_document {
                return false;
            }

            self.frameset_element = find_direct_html_element(html_element, "frameset");
        }

        self.frameset_element
            .as_ref()
            .is_some_and(element_has_window_load_onload)
    }

    fn body_element(&mut self, html_element: &Handle) -> Option<&Handle> {
        if !self.body_lookup_completed {
            self.body_element = find_direct_html_element(html_element, "body");
            self.body_lookup_completed = true;
        }
        self.body_element.as_ref()
    }

    fn html_element(&mut self, document: &Handle) -> Option<Handle> {
        if self.html_element.is_none() {
            self.html_element = find_html_element(document);
        }
        self.html_element.clone()
    }
}

fn find_html_element(document: &Handle) -> Option<Handle> {
    document.children.borrow().iter().find_map(|node| {
        matches!(&node.data, NodeData::Element { name, .. }
            if name.ns == ns!(html)
                && name.local.as_str().eq_ignore_ascii_case("html"))
        .then(|| node.clone())
    })
}

fn find_direct_html_element(parent: &Handle, local_name: &str) -> Option<Handle> {
    parent.children.borrow().iter().find_map(|node| {
        matches!(&node.data, NodeData::Element { name, .. }
            if name.ns == ns!(html)
                && name.local.as_str().eq_ignore_ascii_case(local_name))
        .then(|| node.clone())
    })
}

fn element_has_window_load_onload(node: &Handle) -> bool {
    let NodeData::Element { name, attrs, .. } = &node.data else {
        return false;
    };
    name.ns == ns!(html)
        && (name.local.as_str().eq_ignore_ascii_case("body")
            || name.local.as_str().eq_ignore_ascii_case("frameset"))
        && attrs.borrow().iter().any(|attribute| {
            attribute
                .name
                .local
                .to_string()
                .eq_ignore_ascii_case("onload")
        })
}

fn has_parent(node: &Handle, expected_parent: &Handle) -> bool {
    let parent = node.parent.take();
    node.parent.set(parent.clone());
    parent
        .and_then(|parent| parent.upgrade())
        .is_some_and(|parent| Rc::ptr_eq(&parent, expected_parent))
}

pub(super) fn visible_iframe_count(node: &Handle) -> usize {
    let is_iframe = matches!(&node.data, NodeData::Element { name, .. } if name.local.as_str().eq_ignore_ascii_case("iframe"));
    usize::from(is_iframe)
        + node
            .children
            .borrow()
            .iter()
            .map(visible_iframe_count)
            .sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::{WindowLoadHandlerObserver, element_has_window_load_onload};
    use html5ever::{parse_document, tendril::TendrilSink};
    use markup5ever_rcdom::RcDom;

    #[test]
    fn observer_returns_false_when_document_has_no_html_element() {
        let dom = RcDom::default();
        let mut observer = WindowLoadHandlerObserver::default();

        assert!(!observer.token_created_or_updated_window_load_handler(&dom.document, "body"));
    }

    #[test]
    fn observer_ignores_tokens_that_cannot_set_window_load_handler() {
        let dom = parse_document(RcDom::default(), Default::default()).one("<body onload='run()'>");
        let mut observer = WindowLoadHandlerObserver::default();

        assert!(!observer.token_created_or_updated_window_load_handler(&dom.document, "div"));
    }

    #[test]
    fn observer_reuses_the_cached_frameset_for_repeated_tokens() {
        let dom = parse_document(RcDom::default(), Default::default())
            .one("<frameset onload='run()'></frameset>");
        let mut observer = WindowLoadHandlerObserver::default();

        assert!(observer.token_created_or_updated_window_load_handler(&dom.document, "frameset"));
        assert!(observer.token_created_or_updated_window_load_handler(&dom.document, "frameset"));
    }

    #[test]
    fn non_element_nodes_do_not_have_window_load_handlers() {
        let dom = RcDom::default();

        assert!(!element_has_window_load_onload(&dom.document));
    }
}
