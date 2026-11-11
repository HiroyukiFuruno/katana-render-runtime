use super::super::super::measurement_diagnostic::{FaceMetricPhase, FaceMetricScope};
use super::face_cache::{commit_file_font, font_for_face};
use harfrust::font::{Font, Source, Tables};
use resvg::usvg::fontdb::{Database, ID};
use rustybuzz::ttf_parser::{Face, RawFaceTables};
use skrifa::raw::{FontRef, ReadError, TableProvider};

pub(in super::super) fn cached_face_line_metrics(
    database: &Database,
    id: ID,
    size: f32,
) -> Result<Option<(f32, f32)>, ()> {
    if !super::super::font::memo_usable() {
        return Ok(None);
    }
    let Some(font) = font_for_face(database, id) else {
        return Ok(None);
    };
    let metrics = sorted_blob_directory(database, id, &font)
        .then(|| metrics_from_font(&font, size))
        .flatten();
    if !commit_file_font(database, id, &font) {
        return Err(());
    }
    Ok(metrics)
}

pub(in super::super) fn diagnosed_cached_face_line_metrics(
    database: &Database,
    id: ID,
    size: f32,
) -> Result<Option<(f32, f32)>, ()> {
    if !super::super::font::memo_usable() {
        return Ok(None);
    }
    let detail = FaceMetricScope::new(true);
    let Some(font) = detail.measure(FaceMetricPhase::Acquire, || font_for_face(database, id))
    else {
        return Ok(None);
    };
    let metrics = detail.measure(FaceMetricPhase::Tables, || {
        checked_metrics(database, id, &font, size)
    });
    if !detail.measure(FaceMetricPhase::Commit, || {
        commit_file_font(database, id, &font)
    }) {
        return Err(());
    }
    Ok(metrics)
}

fn checked_metrics(database: &Database, id: ID, font: &Font, size: f32) -> Option<(f32, f32)> {
    sorted_blob_directory(database, id, font)
        .then(|| metrics_from_font(font, size))
        .flatten()
}

fn metrics_from_font(font: &Font, size: f32) -> Option<(f32, f32)> {
    if !non_variable_tables(font.tables()) {
        return None;
    }
    static_library_metrics(font, size).or_else(|| metrics_from_tables(font.tables(), size))
}

fn static_library_metrics(font: &Font, size: f32) -> Option<(f32, f32)> {
    let tables = font.tables();
    /* WHY: head/maxpの旧parserの許容境界はライブラリ間で異なるため維持する。 */
    let head = rustybuzz::ttf_parser::head::Table::parse(tables.head_data()?)?;
    rustybuzz::ttf_parser::maxp::Table::parse(tables.maxp_data()?)?;
    let hhea = tables.hhea().ok()?;
    let os2 = optional_os2(tables)?;
    let metrics = font.metrics();
    if metrics.use_typo_metrics {
        let os2 = rustybuzz::ttf_parser::os2::Table::parse(os2?)?;
        if !os2.use_typographic_metrics() {
            return None;
        }
    } else if hhea.ascender().to_i16() == 0 || hhea.descender().to_i16() == 0 {
        /* WHY: 旧parserは片側ゼロを独立fallbackするためh_lineの組選択と異なる。 */
        return None;
    }
    let line = metrics.h_line()?;
    if line.descender.to_f32() == f32::from(i16::MIN) {
        return None;
    }
    let scale = size / f32::from(head.units_per_em);
    let leading = (line.line_gap.to_f32() * scale).round();
    let before = (leading / 2.0).floor();
    Some((
        (line.ascender.to_f32() * scale).round() + before,
        (-line.descender.to_f32() * scale).round() + leading - before,
    ))
}

fn sorted_blob_directory(database: &Database, id: ID, font: &Font) -> bool {
    let Source::Blob(bytes) = font.source() else {
        return true;
    };
    let Some((_, index)) = database.face_source(id) else {
        return false;
    };
    let Ok(face) = FontRef::from_index(bytes.as_ref(), index) else {
        return false;
    };
    face.table_directory()
        .table_records()
        .windows(2)
        .all(|pair| pair[0].tag() < pair[1].tag())
}

fn metrics_from_tables(tables: &Tables, size: f32) -> Option<(f32, f32)> {
    if !non_variable_tables(tables) {
        return None;
    }
    let face = face_from_tables(tables)?;
    Some(parsed_face_line_metrics(&face, size))
}

pub(in super::super) fn parsed_face_line_metrics(face: &Face<'_>, size: f32) -> (f32, f32) {
    let scale = size / face.units_per_em() as f32;
    let leading = (face.line_gap() as f32 * scale).round();
    let before = (leading / 2.0).floor();
    (
        (face.ascender() as f32 * scale).round() + before,
        (-f32::from(face.descender()) * scale).round() + leading - before,
    )
}

fn non_variable_tables(tables: &Tables) -> bool {
    matches!(tables.fvar(), Err(ReadError::TableIsMissing(_)))
        && matches!(tables.avar(), Err(ReadError::TableIsMissing(_)))
        && matches!(tables.mvar(), Err(ReadError::TableIsMissing(_)))
}

fn face_from_tables(tables: &Tables) -> Option<Face<'_>> {
    let head = tables.head_data()?;
    let hhea = tables.hhea_data()?;
    let maxp = tables.maxp_data()?;
    let os2 = optional_os2(tables)?;
    Face::from_raw_tables(RawFaceTables {
        head,
        hhea,
        maxp,
        os2,
        ..RawFaceTables::default()
    })
    .ok()
}

fn optional_os2(tables: &Tables) -> Option<Option<&[u8]>> {
    match tables.os2() {
        Ok(os2) => Some(Some(os2.offset_data().as_bytes())),
        Err(ReadError::TableIsMissing(_)) => Some(None),
        Err(_) => None,
    }
}

#[cfg(test)]
#[path = "svg_rasterize_text_harfrust_line_metrics_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_text_harfrust_line_metrics_fastpath_tests.rs"]
mod fastpath_tests;
