use super::{ACTIVE_BATCH, FileStamp, WindowsFontSnapshots};
use std::{path::Path, sync::Arc};

pub(in super::super::super) fn enable_windows_font_snapshots() {
    ACTIVE_BATCH.with(|active| {
        if let Some(batch) = active.borrow_mut().as_mut() {
            batch.windows_font_snapshots.get_or_insert_default();
        }
    });
}

pub(in super::super::super) fn windows_font_snapshot(path: &Path) -> Option<Arc<Vec<u8>>> {
    with_windows_font_snapshots(|snapshots| snapshots.read(path)).flatten()
}

pub(in super::super::super) fn windows_font_snapshot_glyph_support(
    path: &Path,
    face_index: u32,
    character: char,
) -> Option<bool> {
    with_windows_font_snapshots(|snapshots| snapshots.glyph_support(path, face_index, character))
        .flatten()
}

pub(in super::super) fn windows_font_snapshot_stamp(path: &Path) -> Option<FileStamp> {
    ACTIVE_BATCH.with(|active| {
        active
            .borrow()
            .as_ref()?
            .windows_font_snapshots
            .as_ref()?
            .stamp(path)
    })
}

#[cfg(test)]
pub(in super::super) fn windows_font_snapshot_glyph_support_diagnostics(
) -> Option<super::windows_snapshots::GlyphSupportDiagnostics> {
    with_windows_font_snapshots(|snapshots| snapshots.glyph_support_diagnostics())
}

fn with_windows_font_snapshots<T>(
    operation: impl FnOnce(&mut WindowsFontSnapshots) -> T,
) -> Option<T> {
    ACTIVE_BATCH.with(|active| {
        active
            .borrow_mut()
            .as_mut()?
            .windows_font_snapshots
            .as_mut()
            .map(operation)
    })
}
