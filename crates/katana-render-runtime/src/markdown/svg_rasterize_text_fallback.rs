use super::font::{font_has_char, matching_fallback_face};
use resvg::usvg;
use std::collections::HashMap;

pub(super) fn html_font_runs(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    text: &str,
) -> Vec<(usvg::fontdb::ID, String)> {
    let mut runs: Vec<(usvg::fontdb::ID, String)> = Vec::new();
    let mut resolved_faces = HashMap::new();
    for character in text.chars() {
        let face_id = *resolved_faces
            .entry((base_face_id, character))
            .or_insert_with(|| {
                if font_has_char(database, base_face_id, character) {
                    base_face_id
                } else {
                    matching_fallback_face(database, base_face_id, character)
                        .unwrap_or(base_face_id)
                }
            });
        if let Some((run_face_id, run)) = runs.last_mut()
            && *run_face_id == face_id
        {
            run.push(character);
        } else {
            runs.push((face_id, character.to_string()));
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::super::font::{font_has_char, matching_font_face};
    use super::html_font_runs;
    use crate::markdown::svg_rasterize::font::{bundled_font_db, html_font_db_for_text};

    #[test]
    fn html_font_fallback_is_scoped_to_the_current_database() {
        let bundled = bundled_font_db();
        let bundled_base = matching_font_face(&bundled, "Noto Sans", 400, false);
        let bundled_runs = bundled_base.map(|base| html_font_runs(&bundled, base, "日"));
        let html = html_font_db_for_text("Noto Sans", "日");
        let html_base = matching_font_face(&html, "Noto Sans", 400, false);
        let used_html_fallback = html_base.is_some_and(|base| {
            !font_has_char(&html, base, '日')
                && html_font_runs(&html, base, "日本日本")
                    .first()
                    .is_some_and(|(fallback, _)| *fallback != base)
        });

        assert!(bundled_runs.is_some_and(|runs| runs.len() == 1));
        assert!(used_html_fallback);
    }
}
