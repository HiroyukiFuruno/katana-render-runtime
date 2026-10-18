use super::super::proof::CreationProof;
use super::super::sources;
use super::super::{HtmlFontRequest, decision_observer};
use super::admit;
use crate::markdown::svg_rasterize::text_shaping;
use crate::markdown::svg_rasterize::{RasterizedSvg, SvgRasterizeOps};
use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;

#[path = "svg_rasterize_html_font_admission_fixture.rs"]
mod fixture;
use fixture::{OwnedFontCopy, copied_arial_database, face_metadata};

const REGULAR_TEXT: &str = "KRR admission";
const STYLE_TEXT: &str = "KRR Bold";
const FILE_SENTINEL: u8 = 0x5a;
const ADMISSION_FONT_SIZE: f32 = 16.0;
const REGULAR_FONT_WEIGHT: u16 = 400;
const UNSUPPORTED_FONT_WEIGHT: u16 = 500;
const BOLD_FONT_WEIGHT: u16 = 700;

fn regular_admitted(database: &Arc<Database>, proof: &CreationProof) -> Option<ID> {
    admitted(database, proof, "Arial", REGULAR_TEXT, REGULAR_FONT_WEIGHT)
}

fn admitted(
    database: &Arc<Database>,
    proof: &CreationProof,
    family: &str,
    text: &str,
    weight: u16,
) -> Option<ID> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        admit(database, proof, family, text, weight, false)
    })
}

fn eager_render(svg: &str, request: &HtmlFontRequest) -> Result<RasterizedSvg, String> {
    let database = Arc::new(sources::build_html_font_db(request));
    let mut options = super::super::super::rasterizer_options_with_font_db(database);
    text_shaping::install_html_font_resolver(&mut options);
    SvgRasterizeOps::rasterize_with_options(svg, 1.0, &options).map_err(|error| error.to_string())
}

fn assert_images_equal(actual: &RasterizedSvg, expected: &RasterizedSvg) {
    assert_eq!(
        (actual.width, actual.height),
        (expected.width, expected.height)
    );
    assert_eq!(actual.rgba, expected.rgba);
    assert!(actual.rgba.chunks_exact(4).any(|pixel| pixel[3] > 0));
}

fn face_metrics(database: &Database, id: ID) -> Result<(u16, i16, i16), String> {
    database
        .with_face_data(id, |data, index| {
            rustybuzz::ttf_parser::Face::parse(data, index)
                .ok()
                .map(|face| (face.units_per_em(), face.ascender(), face.descender()))
        })
        .ok_or_else(|| "face data unavailable".to_string())?
        .ok_or_else(|| "face metrics unavailable".to_string())
}

#[test]
fn first_exact_arial_face_matches_eager_metadata_and_real_render() -> Result<(), String> {
    let request = HtmlFontRequest::from_text("Arial", REGULAR_TEXT);
    let (known, proof) = sources::build_known_html_font_db(&request);
    let known = Arc::new(known);
    let eager = sources::build_html_font_db(&request);
    let id = admitted(&known, &proof, "Arial", REGULAR_TEXT, REGULAR_FONT_WEIGHT)
        .ok_or_else(|| "Arial admission failed".to_string())?;
    let eager_id = text_shaping::known_matching(&eager, "Arial", REGULAR_FONT_WEIGHT, false)
        .ok_or_else(|| "eager Arial query failed".to_string())?;
    assert_eq!(face_metadata(&known, id)?, face_metadata(&eager, eager_id)?);
    assert_eq!(face_metrics(&known, id)?, face_metrics(&eager, eager_id)?);
    assert!(proof.contains(id) && proof.current(&known));
    assert_family_fallbacks_and_styles()?;
    assert_measurement_contract()?;
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="280" height="80"><rect width="280" height="80" fill="white"/><text x="4" y="32" font-family="Arial" font-size="24" font-weight="400">KRR admission</text><text x="4" y="68" font-family="Arial" font-size="24" font-weight="700">KRR Bold</text></svg>"#;
    let actual =
        SvgRasterizeOps::rasterize_html_svg(svg, 1.0).map_err(|error| error.to_string())?;
    assert_images_equal(&actual, &eager_render(svg, &request)?);
    Ok(())
}

fn measured_width_and_cache_status() -> Result<(f32, bool), String> {
    SvgRasterizeOps::with_html_font_render_scope(|| {
        SvgRasterizeOps::measure_html_text_with_cache_status(
            REGULAR_TEXT,
            "Arial",
            ADMISSION_FONT_SIZE,
            REGULAR_FONT_WEIGHT,
            false,
            0.0,
            None,
        )
    })
    .map_err(|error| error.to_string())
}

fn assert_measurement_contract() -> Result<(), String> {
    let (width, shaped) = measured_width_and_cache_status()?;
    let (ascent, descent) = SvgRasterizeOps::html_line_metrics(
        REGULAR_TEXT,
        "Arial",
        ADMISSION_FONT_SIZE,
        REGULAR_FONT_WEIGHT,
        false,
    )
    .ok_or_else(|| "Arial line metrics are unavailable".to_string())?;
    let runs =
        SvgRasterizeOps::html_text_font_runs(REGULAR_TEXT, "Arial", REGULAR_FONT_WEIGHT, false)
            .ok_or_else(|| "Arial font runs are unavailable".to_string())?;
    assert!(
        shaped
            && width.is_finite()
            && width > 0.0
            && ascent.is_finite()
            && descent.is_finite()
            && ascent + descent > 0.0
            && !runs.is_empty()
    );
    Ok(())
}

fn assert_rejected_fallbacks_and_weights(database: &Arc<Database>, proof: &CreationProof) {
    for (family, weight) in [
        ("KRR missing family, Arial", REGULAR_FONT_WEIGHT),
        ("Arial", UNSUPPORTED_FONT_WEIGHT),
    ] {
        assert!(admitted(database, proof, family, REGULAR_TEXT, weight).is_none());
    }
}

fn assert_family_fallbacks_and_styles() -> Result<(), String> {
    let request = HtmlFontRequest::from_text("Arial", STYLE_TEXT);
    let (database, proof) = sources::build_known_html_font_db(&request);
    let database = Arc::new(database);
    assert_rejected_fallbacks_and_weights(&database, &proof);
    let regular = admitted(
        &database,
        &proof,
        "Arial",
        REGULAR_TEXT,
        REGULAR_FONT_WEIGHT,
    )
    .ok_or_else(|| "regular Arial admission failed".to_string())?;
    let bold = admitted(&database, &proof, "Arial", STYLE_TEXT, BOLD_FONT_WEIGHT)
        .ok_or_else(|| "bold Arial admission failed".to_string())?;
    assert_ne!(regular, bold);
    let face_weight = |id| database.face(id).map(|face| face.weight);
    assert_eq!(
        face_weight(regular),
        Some(resvg::usvg::fontdb::Weight(REGULAR_FONT_WEIGHT))
    );
    assert_eq!(
        face_weight(bold),
        Some(resvg::usvg::fontdb::Weight(BOLD_FONT_WEIGHT))
    );
    assert!(proof.contains(regular) && proof.contains(bold));
    Ok(())
}

#[test]
fn xml_entity_and_cdata_non_ascii_text_render_like_eager_fallback() -> Result<(), String> {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="56"><rect width="240" height="56" fill="white"/><text x="4" y="40" font-family="Arial" font-size="32">&#x65E5;<![CDATA[語]]></text></svg>"#;
    let request = HtmlFontRequest::from_markup(svg);
    let (known, proof) = sources::build_known_html_font_db(&request);
    let known = Arc::new(known);
    assert!(admitted(&known, &proof, "Arial", "日語", REGULAR_FONT_WEIGHT).is_none());
    let actual =
        SvgRasterizeOps::rasterize_html_svg(svg, 1.0).map_err(|error| error.to_string())?;
    assert_images_equal(&actual, &eager_render(svg, &request)?);
    Ok(())
}

const INTERACTION_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/html/interaction-fidelity.html"
));

#[test]
fn interaction_fixture_initial_layout_uses_known_arial() -> Result<(), String> {
    let _ = super::super::html_font_db_for_text("Arial", "Toggle panel");
    assert_eq!(
        super::super::measured_database("Arial", "Toggle panel", REGULAR_FONT_WEIGHT, false)
            .key
            .discovery,
        super::super::DiscoveryMode::Eager
    );
    let observer = decision_observer::Scope::enter();
    let source = crate::HtmlBrowserSource::new(
        INTERACTION_FIXTURE,
        "https://example.test/interaction-fidelity.html",
    )
    .map_err(|error| error.to_string())?;
    let viewport =
        crate::HtmlBrowserViewport::new(1280, 900, 1.0).map_err(|error| error.to_string())?;
    let mut session =
        crate::HtmlBrowserSession::new(source, viewport).map_err(|error| error.to_string())?;
    assert!(session.latest_frame().is_some());
    assert_known_interaction_decision(&observer.records())?;
    session.close().map_err(|error| error.to_string())
}

fn assert_known_interaction_decision(
    decisions: &[decision_observer::Decision],
) -> Result<(), String> {
    let decision = decisions
        .iter()
        .find(|decision| {
            decision.family == "Arial"
                && decision.text == "Toggle panel"
                && decision.weight == REGULAR_FONT_WEIGHT
                && !decision.italic
        })
        .ok_or_else(|| "initial interaction layout did not record Arial button text".to_string())?;
    assert_eq!(
        decision.key.discovery,
        super::super::DiscoveryMode::KnownOnly
    );
    assert_eq!(
        decision.key.request,
        HtmlFontRequest::from_text("Arial", "Toggle panel")
    );
    assert!(decision.database.upgrade().is_some());
    Ok(())
}

#[test]
fn changed_owned_real_arial_file_invalidates_known_admission() -> Result<(), String> {
    let (database, proof, copied) = copied_arial_database()?;
    assert!(regular_admitted(&database, &proof).is_some());
    let original_len = std::fs::metadata(&copied.path)
        .map_err(|error| error.to_string())?
        .len();
    copied.append_sentinel(FILE_SENTINEL)?;
    assert!(regular_admitted(&database, &proof).is_none());
    copied.restore_len(original_len)?;
    assert!(regular_admitted(&database, &proof).is_none());
    assert!(!proof.current(&database));
    Ok(())
}

#[path = "svg_rasterize_html_font_admission_boundary_tests.rs"]
mod boundary_tests;
