pub(super) use super::super::super::super::font::font_source_generation;
pub(super) use super::super::super::super::variations::FontShape;
pub(super) use super::super::super::shape_font;
pub(super) use super::super::{ACTIVE_SCOPE, FontHandle, commit_file_font};
use super::Database;

pub(super) const TTC_TEXT: &str = "日本語 ABC";
pub(super) const TTC_SIZE: f32 = 16.0;
pub(super) const TTC_WEIGHT: u16 = 700;
pub(super) const POISON_WEIGHT: u16 = 400;
pub(super) const TRUNCATION_DIVISOR: usize = 2;
pub(super) const SFNT_NUM_TABLES_FIELD_OFFSET: usize = 4;
pub(super) const SFNT_TABLE_DIRECTORY_START: usize = 12;
pub(super) const SFNT_TABLE_RECORD_SIZE: usize = 16;
pub(super) const SFNT_TAG_BYTE_LENGTH: usize = 4;
pub(super) const SFNT_TABLE_OFFSET_FIELD_START: usize = 8;
pub(super) const SFNT_TABLE_OFFSET_FIELD_END: usize = 12;
pub(super) const SFNT_TABLE_LENGTH_FIELD_START: usize = 12;
pub(super) const SFNT_TABLE_LENGTH_FIELD_END: usize = 16;
pub(super) const BACKED_FONT_COUNT: usize = 3;
pub(super) const FIRST_BACKED_FONT_COUNT: usize = 2;
pub(super) const FIRST_BACKED_FONT_BYTES: usize = 20 * 1024 * 1024;
pub(super) const NEXT_BACKED_FONT_BYTES: usize = 40 * 1024 * 1024;

pub(super) fn shape_and_commit(
    database: &Database,
    face: resvg::usvg::fontdb::ID,
    font: &FontHandle,
    label: &str,
) -> Result<(), String> {
    super::super::super::shape_font(
        font,
        TTC_TEXT,
        FontShape::new(TTC_SIZE, TTC_WEIGHT, false),
        Some("kern"),
    )
    .ok_or_else(|| format!("{label} shaping failed"))?;
    if !commit_file_font(database, face, font) {
        return Err(format!("{label} commit failed"));
    }
    Ok(())
}

pub(super) fn assert_all_handles_cleared() {
    ACTIVE_SCOPE.with(|cache| {
        let cache = cache.borrow();
        assert!(cache.entry.is_none() && cache.spare.is_none());
    });
}
