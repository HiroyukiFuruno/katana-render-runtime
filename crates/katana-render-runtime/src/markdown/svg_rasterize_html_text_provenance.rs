#[cfg(any(target_os = "macos", test))]
use super::types::SvgRasterizeError;
#[cfg(any(target_os = "macos", test))]
use resvg::usvg;
#[cfg(any(target_os = "macos", test))]
use sha2::{Digest, Sha256};
#[cfg(any(target_os = "macos", test))]
use std::collections::{HashMap, HashSet};
#[cfg(any(target_os = "macos", test))]
#[path = "svg_rasterize_html_text_provenance_tree.rs"]
mod tree;
#[cfg(any(target_os = "macos", test))]
use tree::tree_ids;

#[derive(Clone, Default)]
pub(crate) struct GeneratedHtmlTextProvenance {
    #[cfg(any(target_os = "macos", test))]
    next_id: u64,
    #[cfg(any(target_os = "macos", test))]
    entries: Vec<GeneratedHtmlTextEntry>,
}

#[cfg(any(target_os = "macos", test))]
#[derive(Clone)]
struct GeneratedHtmlTextEntry {
    id: String,
    markup_sha256: String,
}

impl GeneratedHtmlTextProvenance {
    pub(crate) fn record(&mut self, markup: String) -> String {
        #[cfg(not(any(target_os = "macos", test)))]
        {
            let _ = self;
            return markup;
        }
        #[cfg(any(target_os = "macos", test))]
        return self.record_text_markup(markup);
    }

    #[cfg(any(target_os = "macos", test))]
    fn record_text_markup(&mut self, markup: String) -> String {
        if !markup.starts_with("<text ") {
            return markup;
        }
        let Some(open_end) = markup.find('>') else {
            return markup;
        };
        let start_tag = &markup[..open_end];
        if start_tag.contains(" id=") {
            return markup;
        }
        let Some(next_id) = self.next_id.checked_add(1) else {
            return markup;
        };
        let id = format!("krr-html-text-{next_id}");
        let marked = format!("{start_tag} id=\"{id}\"{}", &markup[open_end..]);
        self.register_marked_text(id, next_id, marked)
    }

    #[cfg(any(target_os = "macos", test))]
    fn register_marked_text(&mut self, id: String, next_id: u64, markup: String) -> String {
        self.next_id = next_id;
        self.entries.push(GeneratedHtmlTextEntry {
            id,
            markup_sha256: sha256(markup.as_bytes()),
        });
        markup
    }

    #[cfg(any(target_os = "macos", test))]
    pub(crate) fn validated_routing(
        &self,
        svg: &str,
        tree: &usvg::Tree,
    ) -> Result<HtmlTextRouting, SvgRasterizeError> {
        if self.entries.is_empty() {
            return Ok(HtmlTextRouting::default());
        }
        let document = usvg::roxmltree::Document::parse(svg)
            .map_err(|error| SvgRasterizeError::ParseFailed(error.to_string()))?;
        Ok(validated_routing(&self.entries, svg, &document, tree))
    }

    #[cfg(test)]
    pub(crate) fn validated_ids(
        &self,
        svg: &str,
        tree: &usvg::Tree,
    ) -> Result<HashSet<String>, SvgRasterizeError> {
        self.validated_routing(svg, tree).map(|routing| routing.ids)
    }
}

#[cfg(any(target_os = "macos", test))]
#[derive(Default)]
pub(crate) struct HtmlTextRouting {
    pub(crate) ids: HashSet<String>,
    pub(crate) all_text_verified: bool,
}

#[cfg(any(target_os = "macos", test))]
fn validated_routing(
    entries: &[GeneratedHtmlTextEntry],
    svg: &str,
    document: &usvg::roxmltree::Document<'_>,
    tree: &usvg::Tree,
) -> HtmlTextRouting {
    let source_ids = source_ids(document);
    let tree_ids = tree_ids(tree);
    let ids = validated_text_ids(entries, svg, &source_ids, &tree_ids);
    let all_text_verified = all_text_verified(entries, &ids, &tree_ids);
    HtmlTextRouting {
        ids,
        all_text_verified,
    }
}

#[cfg(any(target_os = "macos", test))]
fn validated_text_ids(
    entries: &[GeneratedHtmlTextEntry],
    svg: &str,
    source_ids: &HashMap<&str, Vec<SourceNode>>,
    tree_ids: &tree::TreeIds,
) -> HashSet<String> {
    entries
        .iter()
        .filter(|entry| source_text_matches(entry, svg, source_ids))
        .filter(|entry| tree_ids.text_ids.contains(entry.id.as_str()))
        .filter(|entry| {
            tree_ids
                .counts
                .get(&entry.id)
                .is_some_and(|nodes| nodes.len() == 1)
        })
        .map(|entry| entry.id.clone())
        .collect()
}

#[cfg(any(target_os = "macos", test))]
fn all_text_verified(
    entries: &[GeneratedHtmlTextEntry],
    ids: &HashSet<String>,
    tree_ids: &tree::TreeIds,
) -> bool {
    !ids.is_empty()
        && ids.len() == entries.len()
        && !tree_ids.has_unregistered_text
        && tree_ids.text_ids.len() == ids.len()
        && tree_ids.text_ids.iter().all(|id| ids.contains(id))
}

#[cfg(any(target_os = "macos", test))]
struct SourceNode {
    range: std::ops::Range<usize>,
    is_text: bool,
}

#[cfg(any(target_os = "macos", test))]
fn source_ids<'document>(
    document: &'document usvg::roxmltree::Document<'_>,
) -> HashMap<&'document str, Vec<SourceNode>> {
    let mut ids = HashMap::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        if let Some(id) = node.attribute("id") {
            ids.entry(id).or_insert_with(Vec::new).push(SourceNode {
                range: node.range(),
                is_text: node.has_tag_name("text"),
            });
        }
    }
    ids
}

#[cfg(any(target_os = "macos", test))]
fn source_text_matches(
    entry: &GeneratedHtmlTextEntry,
    svg: &str,
    source_ids: &HashMap<&str, Vec<SourceNode>>,
) -> bool {
    let Some([node]) = source_ids.get(entry.id.as_str()).map(Vec::as_slice) else {
        return false;
    };
    node.is_text
        && svg
            .get(node.range.clone())
            .is_some_and(|markup| sha256(markup.as_bytes()) == entry.markup_sha256)
}

#[cfg(any(target_os = "macos", test))]
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
#[path = "svg_rasterize_html_text_provenance_tests.rs"]
mod tests;
