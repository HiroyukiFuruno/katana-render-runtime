use super::super::measurement_token::{HtmlTextMeasurementToken, HtmlTextMeasurementTokenKey};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::HashMap,
    hash::{Hash, Hasher},
};

const SHA256_DIGEST_BYTES: usize = 32;
const MAX_PERSISTENT_METRICS: usize = 256;

#[derive(Clone, Eq, Hash, PartialEq)]
struct Key {
    text: [u8; SHA256_DIGEST_BYTES],
    family: [u8; SHA256_DIGEST_BYTES],
    size: u32,
    weight: u16,
    italic: bool,
    token: HtmlTextMeasurementTokenKey,
}

pub(super) struct Request {
    key: Key,
    token: HtmlTextMeasurementToken,
}

impl Request {
    pub(super) fn new(
        text: &str,
        family: &str,
        size: f32,
        weight: u16,
        italic: bool,
    ) -> Option<Self> {
        if text.is_empty() {
            return None;
        }
        let token = HtmlTextMeasurementToken::for_measured_text(text, family, weight, italic)
            .filter(HtmlTextMeasurementToken::is_usable)?;
        let key = Key {
            text: Sha256::digest(text.as_bytes()).into(),
            family: Sha256::digest(family.as_bytes()).into(),
            size: size.to_bits(),
            weight,
            italic,
            token: token.key(),
        };
        Some(Self { key, token })
    }
}

thread_local! {
    /* WHY: 成功値とWeak proofだけを保持し、font source/DBの寿命は延ばさない。 */
    static VALUES: RefCell<HashMap<Key, (f32, f32)>> = RefCell::new(HashMap::new());
}

pub(super) fn lookup(request: &Request) -> Option<(f32, f32)> {
    if !request.token.is_usable() {
        return None;
    }
    VALUES.with(|cache| cache.borrow().get(&request.key).copied())
}

pub(super) fn remember(request: &Request, metrics: (f32, f32)) {
    if !request.token.is_usable() {
        return;
    }
    VALUES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= MAX_PERSISTENT_METRICS
            && !cache.contains_key(&request.key)
            && let Some(evicted) = cache.keys().next().cloned()
        {
            cache.remove(&evicted);
        }
        cache.insert(request.key.clone(), metrics);
    });
}

#[cfg(test)]
#[path = "svg_rasterize_html_line_metrics_persistent_tests.rs"]
mod tests;

pub(super) fn with_reuse(
    input: (&str, &str, f32, u16, bool),
    measure: impl FnOnce() -> Option<(f32, f32)>,
) -> Option<(f32, f32)> {
    let (text, family, size, weight, italic) = input;
    let request = Request::new(text, family, size, weight, italic);
    if let Some(metrics) = request.as_ref().and_then(lookup) {
        super::super::super::measurement_diagnostic::line_finish(true, None);
        return Some(metrics);
    }
    let metrics = measure()?;
    if let Some(request) = request.as_ref() {
        remember(request, metrics);
    }
    Some(metrics)
}

pub(super) fn stable_hash(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
