use super::super::html_document::HtmlDocumentNode;
use std::collections::HashMap;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub(super) struct FlowMeasurementKey {
    node_id: u64,
    width_bits: u32,
    list_item_container: bool,
}

impl FlowMeasurementKey {
    pub(super) fn new(node_id: u64, width: f32, list_item_container: bool) -> Self {
        Self {
            node_id,
            width_bits: width.to_bits(),
            list_item_container,
        }
    }

    pub(super) fn for_node(
        node: &HtmlDocumentNode,
        width: f32,
        list_item_container: bool,
    ) -> Option<Self> {
        match node {
            HtmlDocumentNode::Element { node_id, .. } => {
                Some(Self::new(*node_id, width, list_item_container))
            }
            HtmlDocumentNode::Text(_) => None,
        }
    }
}

#[derive(Default)]
pub(super) struct FlowMeasurementCache {
    values: HashMap<FlowMeasurementKey, f32>,
    hits: usize,
    misses: usize,
}

impl FlowMeasurementCache {
    pub(super) fn clear(&mut self) {
        self.values.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub(super) fn get(&mut self, key: FlowMeasurementKey) -> Option<f32> {
        let value = self.values.get(&key).copied();
        if value.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        value
    }

    pub(super) fn insert(&mut self, key: FlowMeasurementKey, height: f32) {
        self.values.insert(key, height);
    }

    pub(super) fn stats(&self) -> (usize, usize) {
        (self.hits, self.misses)
    }
}
