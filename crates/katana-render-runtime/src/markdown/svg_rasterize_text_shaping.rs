#[cfg(test)]
pub(super) use super::font_file_fixture;

#[cfg(target_os = "macos")]
pub(super) use font::{FileStamp, observe_path_stamp, stamp_path_uncached};
#[cfg(target_os = "macos")]
#[path = "svg_rasterize_text_known_admission_bridge.rs"]
mod known_admission;
#[cfg(target_os = "macos")]
pub(super) use known_admission::{
    matching as known_matching, names as known_family_names, observe_all as known_observe_all,
    supports as known_supports, usable as known_memo_usable,
};
#[path = "svg_rasterize_html_input_metrics.rs"]
mod input_metrics;
pub(crate) use input_metrics::InputCharacterMetrics;
pub(super) use input_metrics::html_input_character_metrics;

#[path = "svg_rasterize_text_dx.rs"]
mod dx;
#[cfg(test)]
pub(super) use dx::shaped_html_text_dx;
pub(super) use dx::shaped_html_text_dx_with_trim;

#[path = "svg_rasterize_text_features.rs"]
mod features;
pub(crate) use features::TextSpacingTrim;

#[path = "svg_rasterize_html_line_metrics.rs"]
mod line_metrics;
pub(super) use line_metrics::{
    clear_html_line_metrics_cache, html_line_metrics, html_text_baseline_for_text,
};

#[path = "svg_rasterize_text_fallback_memo.rs"]
mod fallback_memo;
#[path = "svg_rasterize_text_usvg_cmap.rs"]
mod usvg_cmap;

pub(super) use super::font::HtmlFontRequest;
pub(super) use super::font::html_font_db_for_measured_text;
use resvg::usvg;
#[path = "svg_rasterize_text_variations.rs"]
mod variations;
pub(in crate::markdown::svg_rasterize) use variations::{FontShape, effective_html_font_size};

#[path = "svg_rasterize_text_shaping_cache.rs"]
mod cache;
#[path = "svg_rasterize_text_fallback.rs"]
mod fallback;
#[path = "svg_rasterize_text_font.rs"]
mod font;
use font::{FontSourceGeneration, font_source_generation};
#[path = "svg_rasterize_text_measurement_token.rs"]
mod measurement_token;
#[path = "svg_rasterize_text_weight.rs"]
mod weight;
pub(crate) use measurement_token::{HtmlTextMeasurementToken, HtmlTextMeasurementTokenKey};

use cache::cached_shaped_run_with_trim;
pub(crate) use harfrust::SvgTextRunMetrics;
#[path = "svg_rasterize_text_family_runs.rs"]
mod family_runs;
#[path = "svg_rasterize_text_family_scope_cache.rs"]
mod family_scope_cache;
pub(super) use fallback_memo::{install as install_html_fallback_memo, with_validated_tree_parse};
use family_runs::resolved_font_runs;
use font::matching_font_face;
#[cfg(test)]
pub(super) use tests::diagnostic_resolved_font_runs;
pub(super) use usvg_cmap::install as install_html_font_resolver;
pub(super) use usvg_cmap::loaded_html_font;

pub(super) fn with_font_face_cache_scope<T>(operation: impl FnOnce() -> T) -> T {
    /* WHY: 同一frameのlayoutからraster完了までlibrary-owned metricsを共有する。 */
    let _font_scope = harfrust::face_cache::FontFaceCacheScope::enter();
    operation()
}

pub(super) fn with_render_scoped_stamp_batch<T>(operation: impl FnMut() -> T) -> T {
    let mut operation = operation;
    font::with_render_scoped_stamp_batch(|| {
        let _family_scope = family_scope_cache::Scope::enter();
        let _font_scope = harfrust::face_cache::FontFaceCacheScope::enter();
        let _token_scope = measurement_token::Scope::enter();
        operation()
    })
}

#[cfg(test)]
pub(super) fn shaped_text_width(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    shaped_text_width_with_trim(
        text,
        font_family,
        FontShape::new(font_size, font_weight, italic),
        letter_spacing,
        font_feature_settings,
    )
}

pub(super) fn shaped_text_width_with_trim(
    text: &str,
    font_family: &str,
    font: FontShape,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    let database = html_font_db_for_measured_text(font_family, text, font.weight, font.italic);
    shaped_text_width_in_database_with_trim(
        &database,
        text,
        font_family,
        font,
        letter_spacing,
        font_feature_settings,
    )
}

pub(super) fn shaped_text_width_in_database_with_trim(
    database: &std::sync::Arc<usvg::fontdb::Database>,
    text: &str,
    font_family: &str,
    font: FontShape,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    let base_face_id = matching_font_face(database, font_family, font.weight, font.italic)?;
    let advance = shaped_runs_advance(
        database,
        base_face_id,
        font_family,
        text,
        font,
        font_feature_settings,
    )?;
    let spacing = text.chars().count().saturating_sub(1) as f32 * letter_spacing;
    Some(advance + spacing)
}

fn shaped_runs_advance(
    database: &std::sync::Arc<usvg::fontdb::Database>,
    base_face_id: usvg::fontdb::ID,
    font_family: &str,
    text: &str,
    shape: FontShape,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    let runs = resolved_font_runs(
        database,
        base_face_id,
        font_family,
        text,
        shape.weight,
        shape.italic,
    );
    #[cfg(all(test, target_os = "macos"))]
    for (face_id, _) in &runs {
        super::font::record_actual_selected_face(database, Some(*face_id));
    }
    advance_runs(database, runs, shape, font_feature_settings)
}

fn advance_runs(
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

pub(super) fn html_text_font_runs(
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

#[path = "svg_rasterize_text_harfrust.rs"]
mod harfrust;
#[cfg(target_os = "macos")]
pub(in crate::markdown::svg_rasterize) use harfrust::eligible_immutable_system_path;
#[path = "svg_rasterize_text_shaping_svg_metrics.rs"]
mod svg_metrics;
pub(super) use svg_metrics::svg_dom_layout_ascent;
#[path = "svg_rasterize_text_shaping_x_height.rs"]
mod svg_x_height;
pub(super) use svg_metrics::shaped_svg_text_metrics;
pub(super) use svg_x_height::shaped_svg_font_x_height;

#[cfg(test)]
pub(super) use super::font::html_font_db_for_text;

#[cfg(test)]
#[path = "svg_rasterize_text_shaping_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "macos"))]
pub(super) use measurement_token::with_selection_for_coverage;

#[cfg(all(test, target_os = "macos"))]
pub(in crate::markdown::svg_rasterize) use harfrust::face_cache::observation::{
    source_get as observe_source_cache_get, state as font_cache_observation_state,
};
