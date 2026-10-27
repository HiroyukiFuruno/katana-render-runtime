use std::collections::{HashMap, HashSet};

use resvg::usvg::roxmltree::{Document, Node};

#[path = "svg_rasterize_composite_authored_text_css.rs"]
mod css;
#[path = "svg_rasterize_composite_authored_text_context_source.rs"]
mod source;

#[derive(Default)]
pub(in crate::markdown::svg_rasterize) struct AuthoredTextContext {
    unique_text: HashSet<String>,
    additional: Vec<source::AdditionalTextProof>,
}

impl AuthoredTextContext {
    pub(in crate::markdown::svg_rasterize) fn matches(&self, text: &resvg::usvg::Text) -> bool {
        if self.unique_text.is_empty() {
            return false;
        }
        if let [chunk] = text.chunks() {
            return self.matches_text(chunk.text());
        }
        false
    }

    pub(super) fn matches_text(&self, text: &str) -> bool {
        self.unique_text.contains(text)
    }

    pub(in crate::markdown::svg_rasterize) fn matches_additional(
        &self,
        text: &resvg::usvg::Text,
    ) -> bool {
        self.additional.iter().any(|proof| proof.matches(text))
    }
}

pub(super) fn collect(document: &Document<'_>) -> AuthoredTextContext {
    if source::has_use_element(document) {
        return AuthoredTextContext::default();
    }
    let Some(styles) = source::stylesheet(document) else {
        return AuthoredTextContext::default();
    };
    let text_nodes = document
        .descendants()
        .filter(|node| node.is_element() && node.has_tag_name("text"))
        .collect::<Vec<_>>();
    let text_counts = source::text_occurrences(&text_nodes);
    let mut context = AuthoredTextContext::default();
    for root in text_nodes {
        collect_root(&mut context, root, &styles, &text_counts);
    }
    context
}

fn collect_root(
    context: &mut AuthoredTextContext,
    root: Node<'_, '_>,
    styles: &[simplecss::Selector<'_>],
    counts: &HashMap<String, usize>,
) {
    if let Some(text) = source::nested_single_line_text(root, styles)
        && counts.get(&text) == Some(&1)
    {
        context.unique_text.insert(text);
    }
    /* WHY: 追加laneは原DOMの明示複数行を別に証明し、既存nested証明を迂回させない。 */
    if let Some(proof) = source::additional_text(root, styles)
        && counts.get(proof.source_text()) == Some(&1)
    {
        context.additional.push(proof);
    }
}
