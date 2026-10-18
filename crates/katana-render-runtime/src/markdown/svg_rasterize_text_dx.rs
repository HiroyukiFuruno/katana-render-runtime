use super::super::font::html_font_db_for_measured_text;
use super::FontShape;
use super::family_runs::resolved_font_runs;
use super::font::matching_font_face;
use super::harfrust::shape_run_character_metrics;
use resvg::usvg;

struct TextDxState {
    values: Vec<f32>,
    cumulative_advance_delta: f32,
    current_shift: f32,
}

pub(in super::super) fn shaped_html_text_dx(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    font_feature_settings: Option<&str>,
) -> Option<Vec<f32>> {
    font_feature_settings?;
    let database = html_font_db_for_measured_text(font_family, text, font_weight, italic);
    let base_face_id = matching_font_face(&database, font_family, font_weight, italic)?;
    let mut state = TextDxState::new(text.chars().count());
    for (face_id, run) in resolved_font_runs(
        &database,
        base_face_id,
        font_family,
        text,
        font_weight,
        italic,
    ) {
        state.append_run(
            &database,
            face_id,
            &run,
            FontShape::new(font_size, font_weight, italic),
            font_feature_settings,
        )?;
    }
    state.finish()
}

impl TextDxState {
    fn new(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            cumulative_advance_delta: 0.0,
            current_shift: 0.0,
        }
    }

    fn append_run(
        &mut self,
        database: &usvg::fontdb::Database,
        face_id: usvg::fontdb::ID,
        run: &str,
        font: FontShape,
        font_feature_settings: Option<&str>,
    ) -> Option<()> {
        let natural = shape_run_character_metrics(database, face_id, run, font, None)?;
        let featured =
            shape_run_character_metrics(database, face_id, run, font, font_feature_settings)?;
        for (natural, featured) in natural.into_iter().zip(featured) {
            let desired_shift = self.cumulative_advance_delta + featured.1 - natural.1;
            self.values.push(desired_shift - self.current_shift);
            self.current_shift = desired_shift;
            self.cumulative_advance_delta += featured.0 - natural.0;
        }
        Some(())
    }

    fn finish(self) -> Option<Vec<f32>> {
        self.values
            .iter()
            .any(|value| value.abs() > f32::EPSILON)
            .then_some(self.values)
    }
}
