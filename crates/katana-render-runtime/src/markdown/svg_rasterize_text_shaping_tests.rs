use super::features::harfrust_features;
use super::font::{
    font_has_char, font_runs, fontdb_family, matching_fallback_face, matching_font_face,
};
use super::{FontShape, shaped_html_text_dx};
use crate::markdown::svg_rasterize::font::{bundled_font_db, html_font_db_for_text};
use resvg::usvg;

#[test]
fn html_text_measurement_uses_per_character_font_fallback() {
    let database = html_font_db_for_text("Noto Sans", "A日本B");
    let used_fallback = matching_font_face(&database, "Noto Sans", 400, false).and_then(|base| {
        matching_fallback_face(&database, base, '日', 400, false).map(|fallback| {
            !font_has_char(&database, base, '日')
                && font_has_char(&database, fallback, '日')
                && font_runs(&database, base, "A日本B").len() == 3
        })
    });
    assert_eq!(used_fallback, Some(true));

    let bundled_database = bundled_font_db();
    let bundled_only =
        matching_font_face(&bundled_database, "Noto Sans", 400, false).is_some_and(|base| {
            matching_fallback_face(&bundled_database, base, '日', 400, false).is_none()
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
        matching_fallback_face(&database, face_id, '日', 400, false).is_none()
            && !font_has_char(&database, face_id, 'A')
            && super::harfrust::shape_text_with_face(
                &database,
                face_id,
                "A",
                FontShape::new(16.0, 400, false),
                None,
            )
            .is_none()
    });
    assert!(removed_face_is_rejected);
}

#[test]
fn shaped_width_preserves_css_letter_spacing_and_empty_runs() -> Result<(), String> {
    let natural = super::shaped_text_width("AV", "Noto Sans", 16.0, 400, false, 0.0, None)
        .ok_or("natural run must shape")?;
    let spaced = super::shaped_text_width("AV", "Noto Sans", 16.0, 400, false, 2.0, None)
        .ok_or("spaced run must shape")?;
    assert!((spaced - natural - 2.0).abs() < 0.001);
    assert_eq!(
        super::shaped_text_width("", "Noto Sans", 16.0, 400, false, 2.0, None),
        Some(0.0)
    );
    assert!(shaped_html_text_dx("אב", "Noto Sans", 16.0, 400, false, Some("\"kern\" 0")).is_none());
    Ok(())
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
    assert!(harfrust_features(None).is_empty());
    assert_eq!(harfrust_features(Some("'liga', 'kern' 0")).len(), 2);
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
        dx.as_ref()
            .is_some_and(|dx| { dx.len() == "（JSC 案件）".chars().count() && dx[0] < 0.0 })
    );
}

#[test]
fn character_metrics_reject_right_to_left_runs() {
    let database = html_font_db_for_text("Noto Sans", "مرحبا");
    let metrics = matching_font_face(&database, "Noto Sans", 400, false)
        .and_then(|face_id| {
            database.with_face_data(face_id, |data, face_index| {
                super::harfrust::shape_face_character_metrics(
                    data,
                    face_index,
                    "مرحبا",
                    FontShape::new(16.0, 400, false),
                    None,
                )
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
                    collect_rustybuzz_character_metrics(&glyphs, "a", 1.0)
                        .map(|metrics| (glyph_count, metrics))
                })
            })
            .flatten()
    });

    assert!(metrics.is_some_and(|(glyph_count, metrics)| {
        glyph_count > 1 && metrics.len() == 1 && metrics[0].0 > 0.0
    }));
}

fn collect_rustybuzz_character_metrics(
    glyphs: &rustybuzz::GlyphBuffer,
    text: &str,
    scale: f32,
) -> Option<Vec<(f32, f32)>> {
    super::harfrust::collect_glyph_character_metrics(text, scale, || {
        glyphs
            .glyph_infos()
            .iter()
            .zip(glyphs.glyph_positions())
            .map(|(info, position)| (info.cluster, position.x_advance, position.x_offset))
    })
}

pub(in crate::markdown::svg_rasterize) fn diagnostic_resolved_font_runs(
    database: &std::sync::Arc<usvg::fontdb::Database>,
    base: usvg::fontdb::ID,
    family: &str,
    text: &str,
    weight: u16,
    italic: bool,
) -> Vec<(usvg::fontdb::ID, String)> {
    super::resolved_font_runs(database, base, family, text, weight, italic)
}
