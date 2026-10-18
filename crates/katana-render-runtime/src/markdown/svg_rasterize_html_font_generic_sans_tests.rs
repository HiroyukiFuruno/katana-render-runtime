use super::super::{DiscoveryMode, HtmlFontRequest, decision_observer, sources};
use super::admit;
use crate::markdown::svg_rasterize::{SvgRasterizeOps, text_shaping};
use resvg::usvg::fontdb::Family;
use std::sync::Arc;

#[path = "svg_rasterize_html_font_generic_sans_test_support.rs"]
mod support;
use support::{
    admitted, assert_empty_admission_rejections, assert_empty_measurement_pair, assert_empty_style,
    assert_nonempty_latin_metrics, assert_same_face, check_aggregate_faces_and_metrics,
    check_initial_generic_fixture, eager_render, initial_frame_sha, interaction_session,
    known_database, metrics, print_font_decisions, print_initial_frame, record_raster_phase,
};

const TEXT: &str = "Open linked page";
const FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/html/interaction-fidelity.html"
));

#[test]
fn first_generic_sans_platform_face_matches_eager_metrics_and_real_render() -> Result<(), String> {
    let (known, proof) = known_database();
    let known = Arc::new(known);
    let eager = sources::build_html_font_db(&HtmlFontRequest::from_text("sans-serif", TEXT));
    assert_eq!(
        known.family_name(&Family::SansSerif),
        sources::configured_html_sans_serif_family()
    );
    assert_eq!(
        known.family_name(&Family::SansSerif),
        eager.family_name(&Family::SansSerif)
    );
    for (weight, italic) in [(400, false), (700, false), (400, true), (700, true)] {
        assert_generic_sans_style_matches_eager(&known, &proof, &eager, weight, italic)?;
    }
    Ok(())
}

fn assert_generic_sans_style_matches_eager(
    known: &Arc<resvg::usvg::fontdb::Database>,
    proof: &super::super::proof::CreationProof,
    eager: &resvg::usvg::fontdb::Database,
    weight: u16,
    italic: bool,
) -> Result<(), String> {
    let selected =
        admitted(known, proof, TEXT, weight, italic).ok_or("generic Arial admission failed")?;
    let eager_id = text_shaping::known_matching(eager, "sans-serif", weight, italic)
        .ok_or("eager generic query failed")?;
    assert_same_face(known, selected, eager, eager_id)?;
    assert_eq!(metrics(known, selected)?, metrics(eager, eager_id)?);
    assert!(proof.contains(selected) && proof.current(known));
    let style = if italic { "italic" } else { "normal" };
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="260" height="60"><rect width="260" height="60" fill="white"/><text x="4" y="34" font-family="sans-serif" font-size="16" font-weight="{weight}" font-style="{style}">Open linked page</text></svg>"#
    );
    let actual =
        SvgRasterizeOps::rasterize_html_svg(&svg, 1.0).map_err(|error| error.to_string())?;
    let expected = eager_render(&svg)?;
    assert_eq!(
        (actual.width, actual.height),
        (expected.width, expected.height)
    );
    assert_eq!(actual.rgba, expected.rgba);
    Ok(())
}

#[test]
fn real_interaction_fixture_initial_generic_sans_layout_uses_known_only() -> Result<(), String> {
    check_initial_generic_fixture()
}

#[test]
fn lower_generic_priority_and_other_generics_remain_unadmitted() {
    for resolved in ["Noto Sans", "Liberation Sans"] {
        let (mut database, proof) = known_database();
        database.set_sans_serif_family(resolved);
        let database = Arc::new(database);
        assert!(proof.current(&database));
        assert!(admitted(&database, &proof, TEXT, 400, false).is_none());
    }
    let (database, proof) = known_database();
    let database = Arc::new(database);
    for family in [
        "serif",
        "monospace",
        "cursive",
        "fantasy",
        "system-ui",
        "serif, Arial",
        "KRR missing family, sans-serif",
    ] {
        assert!(
            SvgRasterizeOps::with_html_font_render_scope(|| admit(
                &database, &proof, family, TEXT, 400, false
            ))
            .is_none(),
            "{family}"
        );
    }
}

#[test]
fn missing_ascii_glyph_and_inexact_style_keep_eager_fallback() -> Result<(), String> {
    let (database, proof) = known_database();
    let database = Arc::new(database);
    let selected =
        admitted(&database, &proof, TEXT, 400, false).ok_or("generic Arial admission failed")?;
    let missing = SvgRasterizeOps::with_html_font_render_scope(|| {
        (0_u8..=127)
            .map(char::from)
            .find(|character| !text_shaping::known_supports(&database, selected, *character))
    })
    .ok_or("real Arial must expose an unsupported ASCII control for the negative case")?;
    assert!(admitted(&database, &proof, &missing.to_string(), 400, false).is_none());
    assert!(admitted(&database, &proof, "日", 400, false).is_none());
    assert!(admitted(&database, &proof, TEXT, 500, false).is_none());
    assert!(admitted(&database, &proof, TEXT, 500, true).is_none());
    Ok(())
}

#[test]
fn later_exact_duplicate_preserves_generic_prefix_winner() -> Result<(), String> {
    let (database, proof) = known_database();
    let database = Arc::new(database);
    let prefix =
        admitted(&database, &proof, TEXT, 400, false).ok_or("generic Arial admission failed")?;
    let face = database.face(prefix).ok_or("prefix face missing")?.clone();
    let mut database = Arc::try_unwrap(database).map_err(|_| "unexpected database owner")?;
    database.push_face_info(face);
    let database = Arc::new(database);
    assert!(proof.current(&database));
    assert_eq!(admitted(&database, &proof, TEXT, 400, false), Some(prefix));
    Ok(())
}

#[test]
fn empty_text_keeps_exact_arial_admission_and_metrics_known_only() -> Result<(), String> {
    SvgRasterizeOps::clear_html_line_metrics_cache();
    let observer = decision_observer::Scope::enter();
    for family in ["Arial", "sans-serif"] {
        let request = HtmlFontRequest::from_text(family, "");
        let (known, proof) = sources::build_known_html_font_db(&request);
        let known = Arc::new(known);
        let eager = Arc::new(sources::build_html_font_db(&request));
        assert!(proof.current(&known));
        for (weight, italic) in [(400, false), (700, false), (400, true), (700, true)] {
            assert_empty_style(
                &observer,
                family,
                &request,
                &known,
                &proof,
                &eager,
                (weight, italic),
            )?;
        }
    }
    let records = observer.records();
    assert_eq!(records.len(), 8, "two family requests × four exact styles");
    assert_empty_admission_rejections();
    Ok(())
}

#[test]
fn empty_measurement_apis_keep_zero_width_empty_runs_and_no_line_metrics() -> Result<(), String> {
    SvgRasterizeOps::clear_html_line_metrics_cache();
    let observer = decision_observer::Scope::enter();
    let mut expected_records = 0usize;
    for family in ["Arial", "sans-serif"] {
        for (weight, italic) in [(400, false), (700, false), (400, true), (700, true)] {
            assert_empty_measurement_pair(
                &observer,
                family,
                weight,
                italic,
                &mut expected_records,
            )?;
        }
    }
    assert_eq!(
        observer.records().len(),
        16,
        "two measured APIs × eight family/style pairs"
    );
    Ok(())
}

#[test]
fn empty_line_metrics_return_none_without_database_selection() {
    SvgRasterizeOps::clear_html_line_metrics_cache();
    let observer = decision_observer::Scope::enter();
    for family in [
        "Arial",
        "sans-serif",
        "serif",
        "monospace",
        "KRR missing family",
    ] {
        for (weight, italic) in [(400, false), (500, false), (700, true)] {
            assert!(
                SvgRasterizeOps::with_html_font_render_scope(|| {
                    SvgRasterizeOps::html_line_metrics("", family, 16.0, weight, italic)
                })
                .is_none()
            );
        }
    }
    assert!(
        observer.records().is_empty(),
        "empty metrics must not choose or construct a database"
    );
}

#[test]
fn nonempty_line_metrics_preserve_spaces_and_unicode_fallback() -> Result<(), String> {
    SvgRasterizeOps::clear_html_line_metrics_cache();
    let observer = decision_observer::Scope::enter();
    for family in ["Arial", "sans-serif"] {
        for (weight, italic) in [(400, false), (700, false), (400, true), (700, true)] {
            assert_nonempty_latin_metrics(family, weight, italic)?;
        }
    }
    let unicode = SvgRasterizeOps::with_html_font_render_scope(|| {
        SvgRasterizeOps::html_line_metrics("日本語", "sans-serif", 16.0, 400, false)
    })
    .ok_or("nonempty Unicode fallback lost metrics")?;
    assert!(unicode.0.is_finite() && unicode.1.is_finite() && unicode.0 + unicode.1 > 0.0);
    let records = observer.records();
    assert!(
        records
            .iter()
            .any(|record| record.text == " " && record.key.discovery == DiscoveryMode::KnownOnly)
    );
    assert!(
        records
            .iter()
            .any(|record| record.text == "日本語" && record.key.discovery == DiscoveryMode::Eager)
    );
    Ok(())
}

#[test]
fn real_interaction_fixture_cold_initial_records_retained_font_decisions() -> Result<(), String> {
    /* WHY: 初回の探索経路を観測するため、事前のfont DB取得を行わない。 */
    let observer = decision_observer::Scope::enter();
    let mut session = interaction_session()?;
    let frame = session
        .latest_frame()
        .ok_or("cold initial fixture session produced no frame")?;
    let pixel_sha256 = initial_frame_sha(frame)?;
    let records = observer.records();
    if records.is_empty() {
        return Err("cold initial fixture produced no font decision records".to_string());
    }
    print_font_decisions(&records);
    print_initial_frame(frame, &pixel_sha256, records.len());
    session.close().map_err(|error| error.to_string())
}

#[test]
fn diagnose_exact_interaction_aggregate_line_feed_faces_and_metrics() -> Result<(), String> {
    check_aggregate_faces_and_metrics()
}

#[test]
fn real_interaction_initial_and_cache_warm_records_retained_raster_font_routes()
-> Result<(), String> {
    for phase in ["initial_session", "cache_warm_second_session"] {
        /* WHY: 実描画の選択だけを観測するため、DB取得や人工SVGを事前に実行しない。 */
        let observer = super::super::raster_observer::Scope::enter();
        let mut session = interaction_session()?;
        let result = record_raster_phase(&session, &observer, phase);
        let closed = session.close().map_err(|error| error.to_string());
        result?;
        closed?;
    }
    Ok(())
}

#[path = "svg_rasterize_html_font_generic_sans_file_shared_diagnostic_tests.rs"]
mod file_shared_stage_diagnostic_tests;
