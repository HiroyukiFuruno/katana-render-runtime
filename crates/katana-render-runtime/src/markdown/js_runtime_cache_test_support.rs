#![cfg(test)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(super) fn with_poisoned_runtime_bundle_cache(
    message: &'static str,
    action: impl FnOnce(&Mutex<HashMap<PathBuf, Arc<str>>>),
) {
    let cache = Mutex::new(HashMap::new());
    let poison = std::panic::catch_unwind(|| poison_runtime_bundle_cache(&cache, message));
    assert!(poison.is_err());
    action(&cache);
    poison_runtime_bundle_cache(&cache, message);
}

fn poison_runtime_bundle_cache(cache: &Mutex<HashMap<PathBuf, Arc<str>>>, message: &'static str) {
    let _guard = match cache.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    std::panic::resume_unwind(Box::new(message));
}
