use super::{
    FontShape, cached_shaped_run_with_trim, html_font_db_for_measured_text, matching_font_face,
    resolved_font_runs, usvg,
};

pub(super) fn advance_runs(
    database: &std::sync::Arc<usvg::fontdb::Database>,
    runs: Vec<(usvg::fontdb::ID, String)>,
    shape: FontShape,
    settings: Option<&str>,
) -> Option<f32> {
    runs.into_iter().try_fold(0.0, |advance, (face_id, run)| {
        cached_shaped_run_with_trim(database, face_id, &run, shape, settings)
            .map(|width| advance + width)
    })
}

pub(in crate::markdown::svg_rasterize) fn html_text_font_runs(
    text: &str,
    font_family: &str,
    font_weight: u16,
    italic: bool,
) -> Option<Vec<(String, String)>> {
    let database = html_font_db_for_measured_text(font_family, text, font_weight, italic);
    let base = matching_font_face(&database, font_family, font_weight, italic)?;
    let runs = resolved_font_runs(&database, base, font_family, text, font_weight, italic);
    runs.into_iter()
        .map(|(face_id, run)| {
            let face = database.face(face_id)?;
            let family = face.families.first()?.0.clone();
            Some((family, run))
        })
        .collect()
}
