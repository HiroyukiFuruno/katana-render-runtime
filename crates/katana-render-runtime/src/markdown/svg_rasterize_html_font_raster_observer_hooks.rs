use super::{ACTIVE, Metric, count, enabled, record_work};
use resvg::usvg;
use sha2::{Digest, Sha256};

pub(in crate::markdown::svg_rasterize) fn eager_tree(
    svg: &str,
    options: &usvg::Options<'_>,
) -> Result<usvg::Tree, usvg::Error> {
    parse(Metric::EagerParse, svg, || {
        usvg::Tree::from_str(svg, options)
    })
}

pub(super) fn parse<T, E>(
    metric: Metric,
    svg: &str,
    operation: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    if !enabled() {
        return operation();
    }
    count(metric);
    let input = serde_json::json!({"kind": metric as usize,
        "bytes": svg.len(), "sha256": super::work::hex(Sha256::digest(svg.as_bytes()))});
    record_work("parse", input.to_string().as_bytes());
    let result = operation();
    record_work(
        "parse_result",
        if result.is_ok() { b"ok" } else { b"error" },
    );
    result
}

pub(in crate::markdown::svg_rasterize) fn source_open(
    path: &std::path::Path,
    stamp: &crate::markdown::svg_rasterize::text_shaping::FileStamp,
) {
    if !enabled() {
        return;
    }
    count(Metric::SourceFirstOpenAttempt);
    let mut hash = Sha256::new();
    hash.update(path.as_os_str().as_encoded_bytes());
    hash.update(format!("{stamp:?}").as_bytes());
    let key = hash.finalize().into();
    ACTIVE.with(|active| {
        if let Some(snapshot) = active.borrow_mut().as_mut() {
            snapshot.work.source_open(key);
        }
    });
}

pub(crate) struct FrameScope {
    previous: Option<u64>,
}
impl FrameScope {
    pub(crate) fn enter() -> Self {
        let previous = ACTIVE.with(|active| match active.borrow_mut().as_mut() {
            Some(snapshot) => Some(snapshot.work.begin_frame()),
            None => None,
        });
        Self { previous }
    }
}

impl Drop for FrameScope {
    fn drop(&mut self) {
        let Some(previous) = self.previous else {
            return;
        };
        let state = crate::markdown::svg_rasterize::text_shaping::font_cache_observation_state();
        ACTIVE.with(|active| {
            if let Some(snapshot) = active.borrow_mut().as_mut() {
                snapshot.counters.cache_state(state.clone());
                snapshot.work.end_frame(state, previous);
            }
        });
    }
}

pub(in crate::markdown::svg_rasterize) fn work_value(domain: &str, value: serde_json::Value) {
    record_work(domain, value.to_string().as_bytes());
}

pub(super) fn route(event: &serde_json::Value) -> Option<Vec<u8>> {
    if !matches!(
        event["stage"].as_str(),
        Some("selector" | "fallback" | "selected_alias")
    ) {
        return None;
    }
    let mut value = event.clone();
    remove_local_ids(&mut value);
    Some(value.to_string().into_bytes())
}

fn remove_local_ids(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            for key in [
                "databaseFaceIdLocalOnly",
                "originalLocalId",
                "aliasLocalId",
                "excludedLocalIds",
            ] {
                object.remove(key);
            }
            for value in object.values_mut() {
                remove_local_ids(value);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                remove_local_ids(value);
            }
        }
        _ => {}
    }
}

pub(in crate::markdown::svg_rasterize) struct AttemptScope {
    previous: Option<u64>,
}

impl AttemptScope {
    pub(in crate::markdown::svg_rasterize) fn enter() -> Self {
        let previous = ACTIVE.with(|active| {
            active.borrow_mut().as_mut().map(|snapshot| {
                snapshot.counters.bump(Metric::AttemptBegin);
                snapshot.work.begin_attempt()
            })
        });
        Self { previous }
    }
}

impl Drop for AttemptScope {
    fn drop(&mut self) {
        let Some(previous) = self.previous else {
            return;
        };
        ACTIVE.with(|active| {
            if let Some(snapshot) = active.borrow_mut().as_mut() {
                snapshot.counters.bump(Metric::AttemptDrop);
                snapshot.work.end_attempt(previous);
            }
        });
    }
}
