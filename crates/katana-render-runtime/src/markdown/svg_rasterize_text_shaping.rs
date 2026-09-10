use super::font::html_font_db_for_text;
use resvg::usvg;

#[path = "svg_rasterize_text_fallback.rs"]
mod fallback;
#[path = "svg_rasterize_text_font.rs"]
mod font;

use fallback::html_font_runs;
use font::matching_font_face;

struct TextDxState {
    values: Vec<f32>,
    cumulative_advance_delta: f32,
    current_shift: f32,
}

pub(super) fn shaped_text_width(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    let database = html_font_db_for_text(font_family, text);
    let base_face_id = matching_font_face(&database, font_family, font_weight, italic)?;
    let mut advance = 0.0;
    for (face_id, run) in html_font_runs(&database, base_face_id, text) {
        advance +=
            shape_text_with_face(&database, face_id, &run, font_size, font_feature_settings)?;
    }
    let spacing = text.chars().count().saturating_sub(1) as f32 * letter_spacing;
    Some(advance + spacing)
}

pub(super) fn shaped_html_text_dx(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    font_feature_settings: Option<&str>,
) -> Option<Vec<f32>> {
    font_feature_settings?;
    let database = html_font_db_for_text(font_family, text);
    let base_face_id = matching_font_face(&database, font_family, font_weight, italic)?;
    let mut state = TextDxState::new(text.chars().count());
    for (face_id, run) in html_font_runs(&database, base_face_id, text) {
        state.append_run(&database, face_id, &run, font_size, font_feature_settings)?;
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
        font_size: f32,
        font_feature_settings: Option<&str>,
    ) -> Option<()> {
        let natural = shape_run_character_metrics(database, face_id, run, font_size, None)?;
        let featured =
            shape_run_character_metrics(database, face_id, run, font_size, font_feature_settings)?;
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

fn shape_text_with_face(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    font_size: f32,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    database
        .with_face_data(face_id, |data, face_index| {
            let face = rustybuzz::Face::from_slice(data, face_index)?;
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(text);
            buffer.guess_segment_properties();
            let glyphs =
                rustybuzz::shape(&face, &rustybuzz_features(font_feature_settings), buffer);
            let advance = glyphs
                .glyph_positions()
                .iter()
                .map(|position| position.x_advance as f32)
                .sum::<f32>();
            Some(advance * font_size / face.units_per_em() as f32)
        })
        .flatten()
}

fn shape_run_character_metrics(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    font_size: f32,
    font_feature_settings: Option<&str>,
) -> Option<Vec<(f32, f32)>> {
    database
        .with_face_data(face_id, |data, face_index| {
            shape_face_character_metrics(data, face_index, text, font_size, font_feature_settings)
        })
        .flatten()
}

fn shape_face_character_metrics(
    data: &[u8],
    face_index: u32,
    text: &str,
    font_size: f32,
    font_feature_settings: Option<&str>,
) -> Option<Vec<(f32, f32)>> {
    let face = rustybuzz::Face::from_slice(data, face_index)?;
    let scale = font_size / face.units_per_em() as f32;
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    if buffer.direction() != rustybuzz::Direction::LeftToRight {
        return None;
    }
    let glyphs = rustybuzz::shape(&face, &rustybuzz_features(font_feature_settings), buffer);
    collect_character_metrics(&glyphs, text, scale)
}

fn collect_character_metrics(
    glyphs: &rustybuzz::GlyphBuffer,
    text: &str,
    scale: f32,
) -> Option<Vec<(f32, f32)>> {
    let character_starts = text
        .char_indices()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let mut metrics = vec![(0.0, 0.0); character_starts.len()];
    let mut seen = vec![false; character_starts.len()];
    for (info, position) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
        let character_index = character_starts
            .binary_search(&(info.cluster as usize))
            .ok()?;
        metrics[character_index].0 += position.x_advance as f32 * scale;
        if !seen[character_index] {
            metrics[character_index].1 = position.x_offset as f32 * scale;
            seen[character_index] = true;
        }
    }
    seen.into_iter().all(|seen| seen).then_some(metrics)
}

fn rustybuzz_features(settings: Option<&str>) -> Vec<rustybuzz::Feature> {
    settings
        .into_iter()
        .flat_map(|settings| settings.split(','))
        .filter_map(|setting| {
            let mut parts = setting.split_whitespace();
            let tag = parts.next()?.trim_matches(['\'', '"']);
            let value = parts.next().unwrap_or("1");
            format!("{tag}={value}").parse().ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::font::{
        font_has_char, font_runs, fontdb_family, matching_fallback_face, matching_font_face,
    };
    use super::{
        collect_character_metrics, rustybuzz_features, shape_face_character_metrics,
        shape_text_with_face, shaped_html_text_dx,
    };
    use crate::markdown::svg_rasterize::font::{bundled_font_db, html_font_db_for_text};
    use resvg::usvg;

    #[test]
    fn html_text_measurement_uses_per_character_font_fallback() {
        let database = html_font_db_for_text("Noto Sans", "A日本B");
        let used_fallback =
            matching_font_face(&database, "Noto Sans", 400, false).and_then(|base| {
                matching_fallback_face(&database, base, '日').map(|fallback| {
                    !font_has_char(&database, base, '日')
                        && font_has_char(&database, fallback, '日')
                        && font_runs(&database, base, "A日本B").len() == 3
                })
            });
        assert_eq!(used_fallback, Some(true));

        let bundled_database = bundled_font_db();
        let bundled_only = matching_font_face(&bundled_database, "Noto Sans", 400, false)
            .is_some_and(|base| {
                matching_fallback_face(&bundled_database, base, '日').is_none()
                    && font_runs(&bundled_database, base, "A日本B").len() == 1
            });
        assert!(bundled_only);
    }

    #[test]
    fn removed_font_face_fails_without_using_stale_font_data() {
        let mut database = (*bundled_font_db()).clone();
        let face_id = matching_font_face(&database, "Noto Sans", 400, false);
        assert!(face_id.is_some(), "bundled Noto Sans must be available");
        let removed_face_is_rejected = face_id.is_some_and(|face_id| {
            database.remove_face(face_id);
            matching_fallback_face(&database, face_id, '日').is_none()
                && !font_has_char(&database, face_id, 'A')
                && shape_text_with_face(&database, face_id, "A", 16.0, None).is_none()
        });
        assert!(removed_face_is_rejected);
    }

    #[test]
    fn generic_families_and_feature_settings_cover_css_shaping_inputs() {
        assert!(matches!(
            fontdb_family("serif"),
            usvg::fontdb::Family::Serif
        ));
        assert!(matches!(
            fontdb_family("cursive"),
            usvg::fontdb::Family::Cursive
        ));
        assert!(matches!(
            fontdb_family("fantasy"),
            usvg::fontdb::Family::Fantasy
        ));
        assert!(rustybuzz_features(None).is_empty());
        assert_eq!(rustybuzz_features(Some("'liga', 'kern' 0")).len(), 2);
    }

    #[test]
    fn shaped_dx_applies_palt_without_expanding_text_nodes() {
        let dx = shaped_html_text_dx(
            "（JSC 案件）",
            "\"Noto Sans JP\", \"Hiragino Sans\", \"Yu Gothic\", sans-serif",
            48.0,
            700,
            false,
            Some(r#""palt" 1"#),
        );
        assert!(
            dx.as_ref().is_some_and(|dx| {
                dx.len() == "（JSC 案件）".chars().count() && dx[0] < 0.0
            })
        );
    }

    #[test]
    fn character_metrics_reject_right_to_left_runs() {
        let database = html_font_db_for_text("Noto Sans", "مرحبا");
        let metrics = matching_font_face(&database, "Noto Sans", 400, false)
            .and_then(|face_id| {
                database.with_face_data(face_id, |data, face_index| {
                    shape_face_character_metrics(data, face_index, "مرحبا", 16.0, None)
                })
            })
            .flatten();

        assert!(metrics.is_none());
    }

    #[test]
    fn character_metrics_reuses_first_offset_for_multi_glyph_cluster() {
        let database = bundled_font_db();
        let metrics = matching_font_face(&database, "Noto Sans", 400, false).and_then(|face_id| {
            database
                .with_face_data(face_id, |data, face_index| {
                    rustybuzz::Face::from_slice(data, face_index).and_then(|face| {
                        let mut buffer = rustybuzz::UnicodeBuffer::new();
                        buffer.add('a', 0);
                        buffer.add('b', 0);
                        let glyphs = rustybuzz::shape(&face, &[], buffer);
                        let glyph_count = glyphs.glyph_infos().len();
                        collect_character_metrics(&glyphs, "a", 1.0)
                            .map(|metrics| (glyph_count, metrics))
                    })
                })
                .flatten()
        });

        assert!(metrics.is_some_and(|(glyph_count, metrics)| {
            glyph_count > 1 && metrics.len() == 1 && metrics[0].0 > 0.0
        }));
    }
}
