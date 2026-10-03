use super::file_generation::{FileStamp, stamp_path_uncached};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
};

const MAX_BATCH_PATHS: usize = 256;

#[derive(Default)]
struct StampBatch {
    stamps: HashMap<PathBuf, FileStamp>,
    invalid: bool,
    overflow: bool,
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
        validate_batch(batch)
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

pub(in super::super) fn with_validated_stamp_batch<T>(mut operation: impl FnMut() -> T) -> T {
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

pub(super) fn cached_stamp(path: &Path) -> Option<FileStamp> {
    ACTIVE_BATCH.with(|active| {
        active
            .borrow()
            .as_ref()
            .and_then(|batch| batch.stamps.get(path).cloned())
    })
}

pub(super) fn remember_stamp(path: &Path, stamp: FileStamp) {
    ACTIVE_BATCH.with(|active| {
        let mut active = active.borrow_mut();
        let Some(batch) = active.as_mut() else {
            return;
        };
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

fn validate_batch(batch: Option<StampBatch>) -> bool {
    let Some(batch) = batch else {
        return false;
    };
    let mut valid = !batch.invalid && !batch.overflow;
    /* WHY: 一時共有した結果を採用する前に、処理中のファイル変更を検出する。 */
    for (path, expected) in batch.stamps {
        if stamp_path_uncached(&path).as_ref() != Ok(&expected) {
            valid = false;
        }
    }
    valid
}

#[cfg(test)]
#[path = "svg_rasterize_text_stamp_batch_tests.rs"]
mod tests;
