use super::super::raster_observer;
use super::{
    Arc, DatabaseKey, DiscoveryMode, HtmlFontRequest, html_font_cache, known_for,
    measured_database, measured_known, usvg,
};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use crate::system::ProcessService;
use std::panic::{catch_unwind, resume_unwind};

const CHILD_CASE: &str = "KRR_HTML_FONT_DISCOVERY_LOCK_TEST_CHILD";
const CASE_NAME: &str = "markdown::svg_rasterize::font::html::discovery_cache::lock_tests::known_font_lookup_and_measurement_recover_from_poisoned_lock";
const MISSING_PROOF_CASE: &str = "markdown::svg_rasterize::font::html::discovery_cache::lock_tests::known_cache_entry_without_proof_falls_back_to_eager_discovery";
const FONT_FAMILY: &str = "Arial";
const FONT_TEXT: &str = "ABC";
const FONT_WEIGHT: u16 = 400;
const FONT_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"><text x="2" y="24" font-family="Arial" font-size="16">ABC</text></svg>"#;

#[test]
fn known_font_lookup_and_measurement_recover_from_poisoned_lock()
-> Result<(), Box<dyn std::error::Error>> {
    if ProcessService::enter_isolated_test_child(CHILD_CASE, CASE_NAME)? {
        verify_recovery()?;
    } else {
        assert!(!html_font_cache().is_poisoned());
    }
    Ok(())
}

fn verify_recovery() -> Result<(), String> {
    let cache = html_font_cache();
    let interrupted = catch_unwind(|| -> Result<(), String> {
        let _guard = cache.lock().map_err(|error| error.to_string())?;
        resume_unwind(Box::new("interrupt isolated HTML font cache access"))
    });
    assert!(interrupted.is_err() && cache.is_poisoned());

    let request = HtmlFontRequest::from_text(FONT_FAMILY, FONT_TEXT);
    let known = known_for(&request).ok_or("known font lookup failed after lock error")?;
    assert!(known.proof.current(&known.database));
    let measured = SvgRasterizeOps::with_html_font_render_scope(|| {
        measured_known(&request, FONT_FAMILY, FONT_TEXT, FONT_WEIGHT, false)
    })
    .ok_or("known font measurement failed after lock error")?;
    assert!(Arc::ptr_eq(&known.database, &measured.database));
    Ok(())
}

#[test]
fn known_cache_entry_without_proof_falls_back_to_eager_discovery()
-> Result<(), Box<dyn std::error::Error>> {
    if ProcessService::enter_isolated_test_child(CHILD_CASE, MISSING_PROOF_CASE)? {
        let request = HtmlFontRequest::from_text(FONT_FAMILY, FONT_TEXT);
        let eager = cache_eager_value_under_known_key(&request)?;
        assert!(known_for(&request).is_none());
        let measured = SvgRasterizeOps::with_html_font_render_scope(|| {
            measured_database(FONT_FAMILY, FONT_TEXT, FONT_WEIGHT, false)
        });
        assert_eq!(measured.key, DatabaseKey::eager(request.clone()));
        assert!(Arc::ptr_eq(&measured.database, &eager));
        verify_raster_fallback(&request)?;
    }
    Ok(())
}

fn verify_raster_fallback(request: &HtmlFontRequest) -> Result<(), String> {
    assert_eq!(&HtmlFontRequest::from_markup(FONT_SVG), request);
    let observer = raster_observer::Scope::enter();
    let image =
        SvgRasterizeOps::rasterize_html_svg(FONT_SVG, 1.0).map_err(|error| error.to_string())?;
    assert!(image.rgba.chunks_exact(4).any(|pixel| pixel[3] != 0));
    let events = observer.snapshot().events;
    assert!(
        events
            .iter()
            .any(|event| event["reason"] == "known_for_unavailable")
    );
    assert!(
        events
            .iter()
            .any(|event| { event["stage"] == "eager_parse_result" && event["success"] == true })
    );
    Ok(())
}

fn cache_eager_value_under_known_key(
    request: &HtmlFontRequest,
) -> Result<Arc<usvg::fontdb::Database>, String> {
    let mut cache = html_font_cache()
        .lock()
        .map_err(|error| error.to_string())?;
    let eager = cache.get_mode(DatabaseKey::eager(request.clone()));
    assert!(eager.proof.is_none());
    let database = Arc::clone(&eager.database);
    let known_key = DatabaseKey {
        request: request.clone(),
        discovery: DiscoveryMode::KnownOnly,
    };
    /* WHY: 通常探索の実データが検証済みのキーに入っても、計測は未検証として扱う。 */
    cache.entries.push_back((known_key, eager));
    Ok(database)
}
