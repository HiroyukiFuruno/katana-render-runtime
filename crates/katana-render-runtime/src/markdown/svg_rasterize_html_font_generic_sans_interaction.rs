use super::super::super::super as html;
use super::super::{FIXTURE, TEXT};
use super::face_assertions::{assert_same_face, metrics};
use crate::markdown::svg_rasterize::text_shaping;
use html::{DiscoveryMode, HtmlFontRequest, decision_observer, sources};
use resvg::usvg::fontdb::{Database, Family};

const VIEWPORT_WIDTH: u32 = 1280;
const VIEWPORT_HEIGHT: u32 = 900;
const NORMAL_FONT_WEIGHT: u16 = 400;
const RGBA_CHANNELS: usize = 4;
const MAX_RASTER_EVENTS: usize = 256;
const MAX_SERIALIZED_EVENT_BYTES: usize = 4096;

pub(in super::super) fn interaction_session() -> Result<crate::HtmlBrowserSession, String> {
    let source =
        crate::HtmlBrowserSource::new(FIXTURE, "https://example.test/interaction-fidelity.html")
            .map_err(|error| error.to_string())?;
    let viewport = crate::HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())?;
    crate::HtmlBrowserSession::new(source, viewport).map_err(|error| error.to_string())
}

fn initial_generic_decision(
    records: &[decision_observer::Decision],
) -> Result<&decision_observer::Decision, String> {
    records
        .iter()
        .find(|decision| {
            decision.family == "sans-serif"
                && decision.text == TEXT
                && decision.weight == NORMAL_FONT_WEIGHT
                && !decision.italic
        })
        .ok_or_else(|| {
            "actual initial link layout did not record the fixture generic family".to_string()
        })
}

fn assert_initial_generic_face(
    database: &Database,
    decision: &decision_observer::Decision,
    eager: &Database,
) -> Result<(), String> {
    assert_eq!(
        database.family_name(&Family::SansSerif),
        sources::configured_html_sans_serif_family()
    );
    let selected =
        text_shaping::known_matching(database, &decision.family, decision.weight, decision.italic)
            .ok_or("actual generic selected face missing")?;
    let eager_id =
        text_shaping::known_matching(eager, &decision.family, decision.weight, decision.italic)
            .ok_or("actual fixture eager selected face missing")?;
    assert_same_face(database, selected, eager, eager_id)?;
    assert_eq!(metrics(database, selected)?, metrics(eager, eager_id)?);
    Ok(())
}

fn frame_pixel_sha(frame: &crate::HtmlBrowserFrame) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(&frame.pixels)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(in super::super) fn initial_frame_sha(
    frame: &crate::HtmlBrowserFrame,
) -> Result<String, String> {
    let expected_bytes = (frame.viewport.width as usize)
        .checked_mul(frame.viewport.height as usize)
        .and_then(|pixels| pixels.checked_mul(RGBA_CHANNELS))
        .ok_or("initial fixture frame dimensions overflow")?;
    if frame.viewport.width == 0
        || frame.viewport.height == 0
        || frame.pixels.len() != expected_bytes
    {
        return Err("initial fixture frame has invalid RGBA dimensions".to_string());
    }
    Ok(frame_pixel_sha(frame))
}

pub(in super::super) fn print_font_decisions(records: &[decision_observer::Decision]) {
    for (sequence, decision) in records.iter().enumerate() {
        println!(
            "KRR_FONT_DECISION {}",
            serde_json::json!({
                "sequence": sequence, "text": decision.text, "family": decision.family,
                "weight": decision.weight, "italic": decision.italic,
                "requestFamilies": decision.key.request.families, "needsCjk": decision.key.request.needs_cjk,
                "needsEmoji": decision.key.request.needs_emoji, "needsUnicodeFallback": decision.key.request.needs_unicode_fallback,
                "discovery": format!("{:?}", decision.key.discovery),
            })
        );
    }
}

pub(in super::super) fn print_initial_frame(
    frame: &crate::HtmlBrowserFrame,
    pixel_sha256: &str,
    decision_count: usize,
) {
    println!(
        "KRR_INITIAL_FRAME {}",
        serde_json::json!({
            "generation": frame.generation, "width": frame.viewport.width, "height": frame.viewport.height,
            "pixelFormat": format!("{:?}", frame.pixel_format), "rgbaBytes": frame.pixels.len(),
            "pixelSha256": pixel_sha256, "decisionCount": decision_count,
        })
    );
}

fn validate_raster_frame(frame: &crate::HtmlBrowserFrame) -> Result<(), String> {
    let expected_bytes = (frame.viewport.width as usize)
        .checked_mul(frame.viewport.height as usize)
        .and_then(|pixels| pixels.checked_mul(RGBA_CHANNELS))
        .ok_or("frame dimensions overflow")?;
    if frame.viewport.width == 0
        || frame.viewport.height == 0
        || frame.pixels.len() != expected_bytes
        || frame.pixel_format != crate::HtmlBrowserPixelFormat::Rgba8
    {
        return Err("fixture frame is not valid RGBA8888".to_string());
    }
    Ok(())
}

fn print_raster_phase(
    frame: &crate::HtmlBrowserFrame,
    phase: &str,
    snapshot: html::raster_observer::Snapshot,
    pixel_sha256: &str,
) {
    println!(
        "KRR_RASTER_FONT_ROUTES {}",
        serde_json::json!({
            "phase": phase, "retainedEvents": snapshot.events, "dropped": snapshot.dropped,
            "maxEvents": MAX_RASTER_EVENTS, "maxSerializedEventBytes": MAX_SERIALIZED_EVENT_BYTES,
            "generation": frame.generation, "width": frame.viewport.width, "height": frame.viewport.height,
            "rgbaBytes": frame.pixels.len(), "pixelSha256": pixel_sha256,
            "physicalPixelsPersisted": false, "timingAcceptance": false,
        })
    );
}

pub(in super::super) fn record_raster_phase(
    session: &crate::HtmlBrowserSession,
    observer: &html::raster_observer::Scope,
    phase: &str,
) -> Result<(), String> {
    let frame = session.latest_frame().ok_or("fixture produced no frame")?;
    validate_raster_frame(frame)?;
    let snapshot = observer.snapshot();
    if snapshot.events.is_empty() {
        return Err("actual raster observer recorded no events".to_string());
    }
    let pixel_sha256 = frame_pixel_sha(frame);
    print_raster_phase(frame, phase, snapshot, &pixel_sha256);
    Ok(())
}

pub(in super::super) fn check_initial_generic_fixture() -> Result<(), String> {
    assert!(FIXTURE.contains("font: 16px sans-serif"));
    let eager = html::html_font_db_for_text("sans-serif", TEXT);
    let observer = decision_observer::Scope::enter();
    let mut session = interaction_session()?;
    assert!(session.latest_frame().is_some());
    let records = observer.records();
    let decision = initial_generic_decision(&records)?;
    assert_eq!(decision.key.discovery, DiscoveryMode::KnownOnly);
    assert_eq!(
        decision.key.request,
        HtmlFontRequest::from_text("sans-serif", TEXT)
    );
    let database = decision
        .database
        .upgrade()
        .ok_or("generic selected database was released")?;
    assert_initial_generic_face(&database, decision, &eager)?;
    session.close().map_err(|error| error.to_string())
}
