use super::super::types::SvgRasterizeError;
use super::tree::{self, tree_ids};
use super::{GeneratedHtmlTextEntry, GeneratedHtmlTextProvenance, ParsedHtmlDocument, sha256};
use resvg::usvg;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct HtmlTextRouting {
    pub(crate) ids: HashSet<String>,
    pub(crate) all_text_verified: bool,
}

impl GeneratedHtmlTextProvenance {
    #[cfg(test)]
    pub(crate) fn validated_routing(
        &self,
        svg: &str,
        tree: &usvg::Tree,
    ) -> Result<HtmlTextRouting, SvgRasterizeError> {
        if self.entries.is_empty() {
            return Ok(HtmlTextRouting::default());
        }
        self.validated_routing_with_document(&usvg::roxmltree::Document::parse(svg), tree)
    }

    pub(crate) fn validated_routing_with_document(
        &self,
        document: &ParsedHtmlDocument<'_>,
        tree: &usvg::Tree,
    ) -> Result<HtmlTextRouting, SvgRasterizeError> {
        if self.entries.is_empty() {
            return Ok(HtmlTextRouting::default());
        }
        let document = document
            .as_ref()
            .map_err(|error| SvgRasterizeError::ParseFailed(error.to_string()))?;
        Ok(validated_routing(
            &self.entries,
            document.input_text(),
            document,
            tree,
        ))
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

struct SourceNode {
    range: std::ops::Range<usize>,
    is_text: bool,
}

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
