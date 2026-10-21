use std::collections::HashMap;

use markup5ever_rcdom::Handle;
use sha2::{Digest, Sha256};

const IMAGE_SOURCE_DIGEST_SIZE: usize = 32;

#[path = "html_document_attributes.rs"]
mod attributes;
#[path = "html_document_mutation.rs"]
mod mutation;
#[path = "html_document_projection.rs"]
mod projection;
use projection::attributes;
#[path = "html_document_form.rs"]
mod form;
#[path = "html_document_parse.rs"]
mod parse;
#[path = "html_document_query.rs"]
mod query;
#[path = "html_document_selector.rs"]
mod selector;
#[path = "html_document_svg.rs"]
mod svg;
#[path = "html_document_tree.rs"]
mod tree;

pub(super) use svg::{
    EMBEDDED_SVG_HEIGHT_PLACEHOLDER, EMBEDDED_SVG_MARKUP_ATTRIBUTE, EMBEDDED_SVG_WIDTH_PLACEHOLDER,
    EMBEDDED_SVG_X_PLACEHOLDER, EMBEDDED_SVG_Y_PLACEHOLDER,
};

/// Canonical HTML5 document state shared by CSS rendering and the V8 bridge.
pub(super) struct HtmlDocument {
    pub(super) document: Handle,
    pub(super) body_onload_script_index: Option<usize>,
    pub(super) body_onload_source: Option<String>,
    pub(super) source_order: Vec<Handle>,
    pub(super) body_onload_source_order_index: Option<usize>,
    pub(super) later_body_onload_tokens: Vec<LaterBodyOnloadToken>,
    nodes: HashMap<u64, Handle>,
    node_ids: HashMap<usize, u64>,
    select_value_empty_overrides: std::collections::HashSet<u64>,
    next_node_id: u64,
    tree_revision: u64,
    /* WHY: 生 URL は大きいため保持せず、作成者属性と独立した固定長キーで実使用量を抑える。 */
    prevalidated_image_events: HashMap<(usize, [u8; IMAGE_SOURCE_DIGEST_SIZE]), &'static str>,
}

pub(super) struct LaterBodyOnloadToken {
    pub(super) script_index: usize,
    pub(super) source_order_index: usize,
    pub(super) source: String,
}

#[derive(Debug, PartialEq)]
pub(super) enum HtmlDocumentScript {
    Source(String),
    LaterBodyOnload(String),
}

impl HtmlDocumentScript {
    pub(super) fn source(&self) -> &str {
        match self {
            Self::Source(source) | Self::LaterBodyOnload(source) => source,
        }
    }

    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Source(_) => "inline-script",
            Self::LaterBodyOnload(_) => "krr-html-later-body-onload",
        }
    }
}

impl LaterBodyOnloadToken {
    pub(super) fn record_source_order(
        tokens: &mut std::iter::Peekable<std::slice::IterMut<'_, Self>>,
        event_index: usize,
        source_order_index: usize,
    ) {
        while let Some(token) = tokens.next_if(|token| token.source_order_index == event_index) {
            token.source_order_index = source_order_index;
        }
    }
}

impl HtmlDocument {
    pub(super) fn clear_prevalidated_image_events(&mut self) {
        self.prevalidated_image_events.clear();
    }

    pub(super) fn cache_prevalidated_image_event(&mut self, source: String, event: &'static str) {
        self.prevalidated_image_events
            .insert(image_source_identity(&source), event);
    }

    pub(super) fn prevalidated_image_event(&self, source: &str) -> Option<&'static str> {
        self.prevalidated_image_events
            .get(&image_source_identity(source))
            .copied()
    }

    pub(crate) fn tree_revision(&self) -> String {
        self.tree_revision.to_string()
    }

    pub(super) fn advance_tree_revision(&mut self) {
        self.tree_revision = self.tree_revision.wrapping_add(1);
    }
}

fn image_source_identity(source: &str) -> (usize, [u8; IMAGE_SOURCE_DIGEST_SIZE]) {
    (source.len(), Sha256::digest(source.as_bytes()).into())
}

/// Dynamic DOM projection used only by KRR's interactive HTML runtime.
///
/// It deliberately retains node IDs so the runtime can perform hit-testing and
/// dispatch input without exposing DOM details to KDV or KatanA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum HtmlDocumentNode {
    Element {
        node_id: u64,
        tag: String,
        attributes: super::html_css::HtmlAttributes,
        children: Vec<HtmlDocumentNode>,
    },
    Text(String),
}

#[cfg(test)]
#[path = "html_document_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "html_document_form_test_support.rs"]
mod form_test_support;

#[cfg(test)]
#[path = "html_document_form_tests.rs"]
mod form_tests;
