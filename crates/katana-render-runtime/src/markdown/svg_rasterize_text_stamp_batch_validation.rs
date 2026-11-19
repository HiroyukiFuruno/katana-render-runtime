use super::super::file_generation::{FileStamp, stamp_path_uncached};
use super::StampBatch;

pub(super) fn validate_batch(batch: Option<StampBatch>) -> bool {
    let Some(batch) = batch else {
        return false;
    };
    let mut valid =
        !batch.invalid && !batch.overflow && batch.stamps.values().all(FileStamp::durable_reusable);
    /* WHY: 一時共有した結果を採用する前に、処理中のファイル変更を検出する。 */
    for (path, expected) in batch.stamps {
        if stamp_path_uncached(&path).as_ref() != Ok(&expected) {
            valid = false;
        }
    }
    #[cfg(windows)]
    if batch
        .windows_font_snapshots
        .as_ref()
        .is_some_and(|snapshots| !snapshots.matches_current())
    {
        valid = false;
    }
    valid
}
