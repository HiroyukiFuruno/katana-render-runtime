use super::super::super::measurement_diagnostic::{self, LineMetricsDetailScope};
use super::{html_font_db_for_measured_text, matching_font_face, resolved_font_runs};
use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

pub(super) fn html_line_metrics_uncached_body(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    detail_enabled: bool,
) -> Option<(f32, f32)> {
    /* WHY: 空本文はglyph runがなく常にNoneなので、結果に不要なfont DBを構築しない。 */
    if text.is_empty() {
        return None;
    }
    let mut detail = measurement_diagnostic::line_metrics_detail(detail_enabled);
    let database = measured_font_database(font_family, text, font_weight, italic, &mut detail);
    let base = matching_line_metric_face(&database, font_family, font_weight, italic, &mut detail)?;
    let runs = resolved_line_metric_runs(
        &database,
        base,
        font_family,
        text,
        font_weight,
        italic,
        &mut detail,
    );
    let (ascent, descent) =
        measure_unique_line_metric_faces(&database, runs, font_size, &mut detail)?;
    (ascent + descent > 0.0).then_some((ascent, descent))
}

fn measured_font_database(
    font_family: &str,
    text: &str,
    font_weight: u16,
    italic: bool,
    detail: &mut Option<LineMetricsDetailScope>,
) -> Arc<Database> {
    if let Some(detail) = detail.as_mut() {
        detail.start_database();
    }
    let database = html_font_db_for_measured_text(font_family, text, font_weight, italic);
    if let Some(detail) = detail.as_mut() {
        detail.finish_phase();
    }
    database
}

fn matching_line_metric_face(
    database: &Database,
    font_family: &str,
    font_weight: u16,
    italic: bool,
    detail: &mut Option<LineMetricsDetailScope>,
) -> Option<ID> {
    if let Some(detail) = detail.as_mut() {
        detail.start_matching();
    }
    let matching = matching_font_face(database, font_family, font_weight, italic);
    if let Some(detail) = detail.as_mut() {
        detail.finish_phase();
    }
    matching
}

fn resolved_line_metric_runs(
    database: &Arc<Database>,
    base: ID,
    font_family: &str,
    text: &str,
    font_weight: u16,
    italic: bool,
    detail: &mut Option<LineMetricsDetailScope>,
) -> Vec<(ID, String)> {
    if let Some(detail) = detail.as_mut() {
        detail.start_runs();
    }
    let runs = resolved_font_runs(database, base, font_family, text, font_weight, italic);
    if let Some(detail) = detail.as_mut() {
        detail.finish_phase();
    }
    runs
}

fn measure_unique_line_metric_faces(
    database: &Database,
    runs: Vec<(ID, String)>,
    font_size: f32,
    detail: &mut Option<LineMetricsDetailScope>,
) -> Option<(f32, f32)> {
    if let Some(detail) = detail.as_mut() {
        detail.start_faces();
    }
    let mut ascent = 0.0_f32;
    let mut descent = 0.0_f32;
    let mut measured_faces = std::collections::HashSet::new();
    for (id, _) in runs {
        if !measured_faces.insert(id) {
            continue;
        }
        record_face_source(database, id, detail);
        let (run_ascent, run_descent) =
            super::observed_line_metrics_for_face(database, id, font_size, detail.is_some())?;
        ascent = ascent.max(run_ascent);
        descent = descent.max(run_descent);
    }
    if let Some(detail) = detail.as_mut() {
        detail.finish_phase();
    }
    Some((ascent, descent))
}

fn record_face_source(database: &Database, id: ID, detail: &mut Option<LineMetricsDetailScope>) {
    if let Some(detail) = detail.as_mut()
        && let Some(face) = database.face(id)
    {
        detail.record_face_source(&face.source);
    }
}
