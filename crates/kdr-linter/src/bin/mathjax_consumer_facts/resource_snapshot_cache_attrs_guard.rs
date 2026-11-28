use syn::visit::Visit;

struct AttributeDetector {
    found: bool,
}

impl<'ast> Visit<'ast> for AttributeDetector {
    fn visit_attribute(&mut self, _: &'ast syn::Attribute) {
        self.found = true;
    }
}

pub(super) fn valid(method: &syn::ImplItemFn) -> bool {
    let mut detector = AttributeDetector { found: false };
    detector.visit_impl_item_fn(method);
    !detector.found
}
