use super::super::{HtmlFontRequest, admission, raster_observer, sources};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use crate::{HtmlBrowserSession, HtmlBrowserSessionState, HtmlBrowserSource, HtmlBrowserViewport};
use resvg::usvg::fontdb::{Database, FaceInfo, Family, Query, Source};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

#[path = "svg_rasterize_html_font_public_route_phases.rs"]
mod phases;

const FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/html/interaction-fidelity.html"
));
const FIXTURE_WIDTH: u32 = 1280;
const FIXTURE_HEIGHT: u32 = 900;

fn fixture_viewport() -> Result<HtmlBrowserViewport, String> {
    HtmlBrowserViewport::new(FIXTURE_WIDTH, FIXTURE_HEIGHT, 1.0).map_err(|e| e.to_string())
}

fn pixel_sha(pixels: &[u8]) -> String {
    Sha256::digest(pixels)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn record_phase(
    phase: &str,
    session: &HtmlBrowserSession,
    observer: &raster_observer::Scope,
) -> Result<(), String> {
    assert_eq!(session.state(), HtmlBrowserSessionState::Active);
    let frame = session.latest_frame().ok_or("fixture frame missing")?;
    assert_eq!((frame.viewport.width, frame.viewport.height), (1280, 900));
    assert_eq!(frame.pixels.len(), 1280 * 900 * 4);
    assert!(frame.pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
    let snapshot = observer.snapshot();
    let (counts, selections) = route_events(&snapshot.events);
    println!(
        "KRR_PUBLIC_FIXTURE_FONT_ROUTES {}",
        serde_json::json!({
            "phase": phase, "eventCounts": counts, "selectorCount": selections.len(),
            "usvgSelectorsObserved": !selections.is_empty(), "selections": selections,
            "events": snapshot.events, "dropped": snapshot.dropped,
            "generation": frame.generation, "scrollY": frame.scroll_y,
            "contentHeight": frame.content_height, "pixelSha256": pixel_sha(&frame.pixels),
            "operationCounters": snapshot.operation_counters(),
            "timingAcceptance": false,
        })
    );
    assert_eq!(snapshot.dropped, 0, "route evidence must be complete");
    assert!(snapshot.complete(), "observer evidence must be complete");
    Ok(())
}

fn route_events(events: &[serde_json::Value]) -> (BTreeMap<String, usize>, Vec<serde_json::Value>) {
    let mut counts = BTreeMap::<String, usize>::new();
    let mut selections = Vec::new();
    for event in events {
        let key = serde_json::json!([event["stage"], event["route"], event["reason"]]).to_string();
        *counts.entry(key).or_default() += 1;
        if event["stage"] == "selector" {
            selections.push(event.clone());
        }
    }
    (counts, selections)
}

#[test]
fn public_fixture_five_phases_observe_raster_font_routes() -> Result<(), String> {
    let viewport = fixture_viewport()?;
    let origin = format!(
        "file:///krr-performance/{}.html",
        pixel_sha(FIXTURE.as_bytes())
    );
    let source = || HtmlBrowserSource::new(FIXTURE, &origin).map_err(|e| e.to_string());
    let mut session = phases::cold_open(&source, viewport)?;
    phases::warm_open(&source, viewport)?;
    phases::re_display(&mut session)?;
    let max_scroll = phases::max_scroll(&session, viewport)?;
    phases::full_y(&mut session, max_scroll)?;
    phases::scroll_top_control(&mut session, max_scroll)?;
    phases::post_operation(&mut session)?;
    phases::close_control(&mut session, "final_close")?;
    assert_eq!(session.state(), HtmlBrowserSessionState::Closed);
    Ok(())
}

#[test]
fn public_phases_reject_operations_after_real_session_closes() -> Result<(), String> {
    let viewport = fixture_viewport()?;
    let source = HtmlBrowserSource::new(FIXTURE, "https://example.test/fixture.html")
        .map_err(|e| e.to_string())?;
    let mut session = crate::HtmlRuntime
        .open(source, viewport)
        .map_err(|e| e.to_string())?;
    session.close().map_err(|e| e.to_string())?;
    assert!(phases::re_display(&mut session).is_err());
    assert_eq!(
        phases::max_scroll(&session, viewport).err().as_deref(),
        Some("initial frame missing")
    );
    assert_eq!(
        phases::full_y(&mut session, 1.0).err().as_deref(),
        Some("scroll frame missing")
    );
    assert_eq!(
        phases::post_operation(&mut session).err().as_deref(),
        Some("operation frame missing")
    );
    assert!(phases::scroll_top_control(&mut session, 1.0).is_err());
    Ok(())
}

#[test]
fn public_open_phases_propagate_source_and_viewport_errors() -> Result<(), String> {
    let malformed_origin = || {
        HtmlBrowserSource::new(FIXTURE, "/missing-scheme.html").map_err(|error| error.to_string())
    };
    let source_error = malformed_origin()
        .err()
        .ok_or("relative origin was accepted")?;
    assert_open_phase_error(&malformed_origin, fixture_viewport()?, &source_error);

    let source = || {
        HtmlBrowserSource::new("<html></html>", "https://example.test/fixture.html")
            .map_err(|error| error.to_string())
    };
    let viewport = HtmlBrowserViewport::new(8193, 1, 1.0).map_err(|error| error.to_string())?;
    let open_error = crate::HtmlRuntime
        .open(source()?, viewport)
        .err()
        .ok_or("viewport beyond the raster width was accepted")?
        .to_string();
    assert_open_phase_error(&source, viewport, &open_error);
    Ok(())
}

fn assert_open_phase_error(
    source: &impl Fn() -> Result<HtmlBrowserSource, String>,
    viewport: HtmlBrowserViewport,
    expected: &str,
) {
    assert_eq!(
        phases::cold_open(source, viewport).err().as_deref(),
        Some(expected)
    );
    assert!(!raster_observer::enabled());
    assert_eq!(
        phases::warm_open(source, viewport).err().as_deref(),
        Some(expected)
    );
    assert!(!raster_observer::enabled());
}

fn face_identity(face: &FaceInfo) -> serde_json::Value {
    let (kind, path) = match &face.source {
        Source::Binary(_) => ("Binary", None),
        Source::File(path) => ("File", Some(path.display().to_string())),
        Source::SharedFile(path, _) => ("SharedFile", Some(path.display().to_string())),
    };
    serde_json::json!({
        "sourceKind": kind, "sourcePath": path, "faceIndex": face.index,
        "families": face.families.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        "postScriptName": face.post_script_name, "weight": face.weight.0,
        "style": format!("{:?}", face.style), "stretch": format!("{:?}", face.stretch),
    })
}

#[test]
fn public_fixture_generic_query_matches_accepted_admission() -> Result<(), String> {
    let request = HtmlFontRequest::from_text("sans-serif", "Open linked page");
    let (known, proof) = sources::build_known_html_font_db(&request);
    let known = Arc::new(known);
    let eager = sources::build_html_font_db(&request);
    let query = Query {
        families: &[Family::SansSerif],
        ..Query::default()
    };
    let known_id = known
        .query(&query)
        .ok_or("known generic query face missing")?;
    let eager_id = eager
        .query(&query)
        .ok_or("eager generic query face missing")?;
    let known_face = face_identity(known.face(known_id).ok_or("known query face missing")?);
    let eager_face = face_identity(eager.face(eager_id).ok_or("eager query face missing")?);
    assert_eq!(known_face, eager_face);
    let admitted = SvgRasterizeOps::with_html_font_render_scope(|| {
        admission::admit(&known, &proof, "sans-serif", "Open linked page", 400, false)
    });
    if let Some(id) = admitted {
        assert_eq!(id, known_id, "accepted admission must match library query");
    }
    record_generic_query(&known, known_face, eager_face, admitted.is_some());
    Ok(())
}

fn record_generic_query(
    known: &Database,
    known_face: serde_json::Value,
    eager_face: serde_json::Value,
    admitted: bool,
) {
    println!(
        "KRR_PUBLIC_FIXTURE_GENERIC_LIBRARY_QUERY {}",
        serde_json::json!({
            "knownFace": known_face, "eagerFace": eager_face,
            "genericFamily": known.family_name(&Family::SansSerif),
            "admitted": admitted, "rasterRouteObserved": false,
            "timingAcceptance": false,
        })
    );
}
