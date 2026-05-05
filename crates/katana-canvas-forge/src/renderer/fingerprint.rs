use super::api::RenderInput;
use std::hash::{Hash, Hasher};

pub(super) struct CacheFingerprintOps;

impl CacheFingerprintOps {
    pub(super) fn render(input: &RenderInput, runtime_version: &str) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        input.kind.hash(&mut hasher);
        input.source.hash(&mut hasher);
        runtime_version.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}
