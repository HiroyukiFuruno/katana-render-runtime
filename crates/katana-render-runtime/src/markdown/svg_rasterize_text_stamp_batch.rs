use super::file_generation::FileStamp;
use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
};

#[cfg(windows)]
#[path = "svg_rasterize_text_stamp_batch_windows_snapshots.rs"]
mod windows_snapshots;
#[cfg(windows)]
use windows_snapshots::WindowsFontSnapshots;
#[cfg(windows)]
#[path = "svg_rasterize_text_stamp_batch_windows_adapter.rs"]
mod windows_adapter;
#[cfg(all(test, windows))]
pub(super) use windows_adapter::windows_font_snapshot_glyph_support_diagnostics;
#[cfg(windows)]
pub(super) use windows_adapter::windows_font_snapshot_stamp;
#[cfg(windows)]
pub(in super::super) use windows_adapter::{
    enable_windows_font_snapshots, windows_font_snapshot, windows_font_snapshot_glyph_support,
};
#[path = "svg_rasterize_text_stamp_batch_validation.rs"]
mod validation;

const MAX_BATCH_PATHS: usize = 256;

#[derive(Default)]
struct StampBatch {
    stamps: HashMap<PathBuf, FileStamp>,
    #[cfg(windows)]
    windows_font_snapshots: Option<WindowsFontSnapshots>,
    invalid: bool,
    overflow: bool,
    memo_disabled: bool,
}

thread_local! {
    static ACTIVE_BATCH: RefCell<Option<StampBatch>> = const { RefCell::new(None) };
}

struct BatchScope {
    previous: Option<StampBatch>,
    nested: bool,
    finished: bool,
}

impl BatchScope {
    fn enter() -> Self {
        /* WHY: 入れ子処理で外側の世代情報を使わないよう、一時共有を解除する。 */
        let previous = ACTIVE_BATCH.with(|active| active.replace(None));
        let nested = previous.is_some();
        if !nested {
            ACTIVE_BATCH.with(|active| active.replace(Some(StampBatch::default())));
        }
        Self {
            previous,
            nested,
            finished: false,
        }
    }

    fn finish(mut self) -> bool {
        if self.nested {
            ACTIVE_BATCH.with(|active| active.replace(self.previous.take()));
            self.finished = true;
            return true;
        }
        let batch = ACTIVE_BATCH.with(|active| active.replace(self.previous.take()));
        self.finished = true;
        validation::validate_batch(batch)
    }
}

impl Drop for BatchScope {
    fn drop(&mut self) {
        if !self.finished {
            ACTIVE_BATCH.with(|active| active.replace(self.previous.take()));
        }
    }
}

struct DisabledBatchScope {
    previous: Option<StampBatch>,
}

impl DisabledBatchScope {
    fn enter() -> Self {
        Self {
            previous: ACTIVE_BATCH.with(|active| active.replace(None)),
        }
    }
}

impl Drop for DisabledBatchScope {
    fn drop(&mut self) {
        ACTIVE_BATCH.with(|active| active.replace(self.previous.take()));
    }
}

pub(in super::super) fn with_render_scoped_stamp_batch<T>(mut operation: impl FnMut() -> T) -> T {
    let scope = BatchScope::enter();
    let result = operation();
    if scope.finish() {
        return result;
    }
    let disabled = DisabledBatchScope::enter();
    let result = operation();
    drop(disabled);
    result
}

pub(in super::super) fn with_reusable_stamp_batch<T>(mut operation: impl FnMut() -> T) -> T {
    if ACTIVE_BATCH.with(|active| active.borrow().is_some()) {
        operation()
    } else {
        with_render_scoped_stamp_batch(operation)
    }
}

pub(in super::super) fn with_validated_stamp_batch<T>(operation: impl FnMut() -> T) -> T {
    with_render_scoped_stamp_batch(operation)
}

pub(super) fn cached_stamp(path: &Path) -> Option<FileStamp> {
    ACTIVE_BATCH.with(|active| {
        active
            .borrow()
            .as_ref()
            .filter(|batch| !batch.memo_disabled)
            .and_then(|batch| batch.stamps.get(path).cloned())
    })
}

pub(super) fn remember_stamp(path: &Path, stamp: FileStamp) {
    ACTIVE_BATCH.with(|active| {
        let mut active = active.borrow_mut();
        let Some(batch) = active.as_mut() else {
            return;
        };
        if batch.memo_disabled {
            return;
        }
        if !stamp.durable_reusable() {
            batch.memo_disabled = true;
            return;
        }
        if batch.stamps.contains_key(path) {
            return;
        }
        if batch.stamps.len() == MAX_BATCH_PATHS {
            batch.overflow = true;
            return;
        }
        batch.stamps.insert(path.to_path_buf(), stamp);
    });
}

pub(super) fn mark_unavailable() {
    ACTIVE_BATCH.with(|active| {
        if let Some(batch) = active.borrow_mut().as_mut() {
            batch.invalid = true;
        }
    });
}

pub(in super::super) fn memo_usable() -> bool {
    ACTIVE_BATCH.with(|active| {
        active
            .borrow()
            .as_ref()
            .is_some_and(|batch| !batch.invalid && !batch.overflow && !batch.memo_disabled)
    })
}

#[cfg(test)]
#[path = "svg_rasterize_text_stamp_batch_tests.rs"]
mod tests;
#[cfg(all(test, windows))]
#[path = "svg_rasterize_text_stamp_batch_windows_snapshot_tests.rs"]
mod windows_snapshot_tests;
