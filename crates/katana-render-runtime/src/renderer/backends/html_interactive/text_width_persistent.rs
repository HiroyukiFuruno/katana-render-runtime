use super::TextMeasurementKey;
use crate::markdown::svg_rasterize::HtmlTextMeasurementTokenKey;
use std::collections::HashMap;

const MAX_PERSISTENT_WIDTHS: usize = 4096;

#[derive(Clone, Eq, Hash, PartialEq)]
pub(super) struct PersistentTextWidthKey {
    measurement: TextMeasurementKey,
    token: HtmlTextMeasurementTokenKey,
}

impl PersistentTextWidthKey {
    pub(super) fn new(measurement: TextMeasurementKey, token: HtmlTextMeasurementTokenKey) -> Self {
        Self { measurement, token }
    }
}

#[derive(Default)]
pub(super) struct PersistentTextWidthCache {
    values: HashMap<PersistentTextWidthKey, f32>,
}

impl PersistentTextWidthCache {
    pub(super) fn get(&self, key: &PersistentTextWidthKey) -> Option<f32> {
        self.values.get(key).copied()
    }

    pub(super) fn insert(&mut self, key: PersistentTextWidthKey, width: f32) {
        if self.values.len() >= MAX_PERSISTENT_WIDTHS
            && !self.values.contains_key(&key)
            && let Some(evicted) = self.values.keys().next().cloned()
        {
            self.values.remove(&evicted);
        }
        self.values.insert(key, width);
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.values.len()
    }

    #[cfg(test)]
    pub(super) fn clear_for_test(&mut self) {
        self.values.clear();
    }
}
