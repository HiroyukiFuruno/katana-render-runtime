use super::super::dx::TextDxState;
use super::super::harfrust::native_variations::{
    shape_run_character_metrics_with_variations_with_trim,
    shape_run_character_metrics_without_features_with_variations,
    shape_text_with_face_variations_with_trim,
};
use super::super::{FontShape, cached_shaped_run_with_trim};
use super::{RunFace, RunPlan, SelectedFace};

impl RunPlan {
    pub(in crate::markdown::svg_rasterize) fn advance(
        &self,
        shape: FontShape,
        settings: Option<&str>,
    ) -> Option<f32> {
        self.runs.iter().try_fold(0.0, |advance, run| {
            let width = match &run.face {
                RunFace::Existing(id) => {
                    cached_shaped_run_with_trim(&self.database, *id, &run.text, shape, settings)
                }
                RunFace::Native(selected) => shape_text_with_face_variations_with_trim(
                    &selected.database,
                    selected.id,
                    &run.text,
                    shape,
                    settings,
                    &selected.variations,
                ),
            }?;
            Some(advance + width)
        })
    }

    pub(in crate::markdown::svg_rasterize) fn dx(
        &self,
        count: usize,
        shape: FontShape,
        settings: Option<&str>,
    ) -> Option<Vec<f32>> {
        let mut state = TextDxState::new(count);
        for run in &self.runs {
            match &run.face {
                RunFace::Existing(id) => {
                    state.append_run(&self.database, *id, &run.text, shape, settings)?
                }
                RunFace::Native(face) => {
                    append_native_metrics(&mut state, face, &run.text, shape, settings)?
                }
            }
        }
        state.finish()
    }
}

fn append_native_metrics(
    state: &mut TextDxState,
    face: &SelectedFace,
    text: &str,
    shape: FontShape,
    settings: Option<&str>,
) -> Option<()> {
    let natural = shape_run_character_metrics_without_features_with_variations(
        &face.database,
        face.id,
        text,
        shape,
        &face.variations,
    )?;
    let featured = shape_run_character_metrics_with_variations_with_trim(
        &face.database,
        face.id,
        text,
        shape,
        settings,
        &face.variations,
    )?;
    state.append_metrics(natural, featured);
    Some(())
}

#[cfg(test)]
#[path = "svg_rasterize_text_native_html_run_metrics_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_text_native_html_run_feature_error_tests.rs"]
mod feature_error_tests;
