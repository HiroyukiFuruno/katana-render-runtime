#[path = "svg_rasterize_html_font_generic_sans_file_shared_diagnostic_audit.rs"]
mod audit;
#[path = "svg_rasterize_html_font_generic_sans_file_shared_diagnostic_support.rs"]
mod support;
use super::super::super::lazy_diagnostic::{self, Capture, FaceSource, Mode, SourceRoute};
use super::super::super::raster_observer;
use super::{FIXTURE, interaction_session};
use audit::{audit_measured_sources, normalized_faces, validate_shared_drop};
use sha2::{Digest, Sha256};
use std::time::Instant;
use support::{
    tree_parse_samples, validate_arm, validate_audit, validate_capture, validate_observer,
};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 900;
const MAX_OBSERVER_EVENTS: usize = 256;
const MAX_SERIALIZED_EVENT_BYTES: usize = 4096;
const EXPECTED_SELECTED_FACES: usize = 3;
const MAX_CAPTURED_STAGES: usize = 512;
const RGBA_CHANNELS: usize = 4;
const PAIR_COUNT: usize = 5;
const PAIR_ORDER_PERIOD: usize = 2;
struct Arm {
    route: SourceRoute,
    pixels: Vec<u8>,
    pixel_sha256: String,
    events: Vec<serde_json::Value>,
    observer_dropped: usize,
    session_ns: u128,
    close_ns: u128,
    drop_ns: u128,
    capture: Capture,
}
struct FrameRecord {
    pixels: Vec<u8>,
    sha: String,
    events: Vec<serde_json::Value>,
    dropped: usize,
    session_ns: u128,
    close_ns: u128,
    drop_ns: u128,
}
fn session(route: SourceRoute, mode: Mode) -> Result<Arm, String> {
    let (result, capture) = lazy_diagnostic::with_route(route, mode, || observe_session(mode));
    let frame = result?;
    validate_capture(mode, &capture)?;
    Ok(Arm {
        route,
        pixels: frame.pixels,
        pixel_sha256: frame.sha,
        events: frame.events,
        observer_dropped: frame.dropped,
        session_ns: frame.session_ns,
        close_ns: frame.close_ns,
        drop_ns: frame.drop_ns,
        capture,
    })
}
fn observe_session(mode: Mode) -> Result<FrameRecord, String> {
    let observer = raster_observer::Scope::enter();
    let started = matches!(mode, Mode::Measure).then(Instant::now);
    let mut session = interaction_session()?;
    let session_ns = elapsed(started);
    let (pixels, sha) = frame_payload(&session)?;
    let close_started = matches!(mode, Mode::Measure).then(Instant::now);
    session.close().map_err(|error| error.to_string())?;
    let close_ns = elapsed(close_started);
    let drop_started = matches!(mode, Mode::Measure).then(Instant::now);
    drop(session);
    let drop_ns = elapsed(drop_started);
    let snapshot = observer.snapshot();
    validate_observer(&snapshot.events)?;
    Ok(FrameRecord {
        pixels,
        sha,
        events: snapshot.events,
        dropped: snapshot.dropped,
        session_ns,
        close_ns,
        drop_ns,
    })
}
fn frame_payload(session: &crate::HtmlBrowserSession) -> Result<(Vec<u8>, String), String> {
    let frame = session.latest_frame().ok_or("fixture produced no frame")?;
    let bytes = (WIDTH as usize)
        .checked_mul(HEIGHT as usize)
        .and_then(|size| size.checked_mul(RGBA_CHANNELS))
        .ok_or("RGBA length overflow")?;
    if frame.viewport.width != WIDTH
        || frame.viewport.height != HEIGHT
        || frame.pixels.len() != bytes
        || frame.pixel_format != crate::HtmlBrowserPixelFormat::Rgba8
    {
        return Err("invalid complete RGBA8 fixture frame".into());
    }
    Ok((frame.pixels.clone(), super::initial_frame_sha(frame)?))
}
fn assert_pair(
    pair: usize,
    fixture_sha: &str,
    left: &Arm,
    right: &Arm,
    audit: &Capture,
) -> Result<(), String> {
    if left.pixels != right.pixels || left.pixel_sha256 != right.pixel_sha256 {
        return Err(format!("full RGBA mismatch in pair {pair}"));
    }
    if normalized_faces(&left.capture) != normalized_faces(&right.capture) {
        return Err(format!("face metadata mismatch in pair {pair}"));
    }
    for arm in [left, right] {
        validate_arm(arm)?;
        audit_measured_sources(audit, &arm.capture)?;
        print_arm(pair, fixture_sha, arm)?;
    }
    Ok(())
}
fn print_arm(pair: usize, fixture_sha: &str, arm: &Arm) -> Result<(), String> {
    let tree_parse_samples = tree_parse_samples(&arm.events)?;
    println!(
        "KRR_FILE_SHARED_STAGE {}",
        serde_json::json!({"pair":pair,"route":format!("{:?}",arm.route),"fixtureSha256":fixture_sha,"stages":arm.capture.stages,"observerEvents":arm.events,"treeParseSamplesNs":tree_parse_samples,"selectedFaces":arm.capture.faces,"databaseWeakCount":arm.capture.database_owners.len(),"actualSelectedDatabaseWeakCount":arm.capture.selected_database_owners.len(),"diagnosticDropped":arm.capture.dropped,"setupDatabaseAlive":arm.capture.database_owners.iter().map(|weak| weak.strong_count()>0).collect::<Vec<_>>(),"actualSelectedDatabaseAlive":arm.capture.selected_database_owners.iter().map(|weak| weak.strong_count()>0).collect::<Vec<_>>(),"sharedBytesAlive":arm.capture.shared_byte_owners.iter().map(|weak| weak.strong_count()>0).collect::<Vec<_>>(),"sharedByteWeakCount":arm.capture.shared_byte_owners.len(),"observerEventLimit":MAX_OBSERVER_EVENTS,"serializedEventByteLimit":MAX_SERIALIZED_EVENT_BYTES,"diagnosticStageLimit":MAX_CAPTURED_STAGES,"observerDropped":arm.observer_dropped,"sessionConstructionAndInitialRasterNs":arm.session_ns,"closeNs":arm.close_ns,"dropNs":arm.drop_ns,"width":WIDTH,"height":HEIGHT,"rgbaBytes":arm.pixels.len(),"pixelSha256":arm.pixel_sha256,"databaseProvision":"uncached File-backed diagnostic fixture; not product cache performance","physicalPixelsPersisted":false,"performanceAcceptance":false,"formal250Replacement":false})
    );
    Ok(())
}
#[test]
fn diagnose_real_interaction_file_vs_sharedfile_stage_costs() -> Result<(), String> {
    let fixture_sha = Sha256::digest(FIXTURE.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let audit_arm = session(SourceRoute::SharedFile, Mode::Audit)?;
    validate_audit(&audit_arm)?;
    let audit = audit_arm.capture;
    if audit.faces.len() != EXPECTED_SELECTED_FACES {
        return Err("untimed audit face count differs from pinned fixture".into());
    }
    for pair in 0..PAIR_COUNT {
        run_pair(pair, &fixture_sha, &audit)?;
    }
    if audit.faces.iter().any(|face| face.bytes_sha256.is_none()) {
        return Err("untimed source-byte audit omitted a digest".into());
    }
    println!(
        "KRR_FILE_SHARED_AUDIT {}",
        serde_json::json!({"fixtureSha256":fixture_sha,"auditedFaces":audit.faces,"sharedTransientDropRequired":true,"formal250Replacement":false,"performanceAcceptance":false})
    );
    Ok(())
}
fn run_pair(pair: usize, fixture_sha: &str, audit: &Capture) -> Result<(), String> {
    let routes = if pair.is_multiple_of(PAIR_ORDER_PERIOD) {
        [SourceRoute::File, SourceRoute::SharedFile]
    } else {
        [SourceRoute::SharedFile, SourceRoute::File]
    };
    let left = session(routes[0], Mode::Measure)?;
    let right = session(routes[1], Mode::Measure)?;
    assert_pair(pair, fixture_sha, &left, &right, audit)
}

fn elapsed(started: Option<Instant>) -> u128 {
    started.map_or(0, |started| started.elapsed().as_nanos())
}

#[path = "svg_rasterize_html_font_file_shared_validator_boundary_tests.rs"]
mod validator_boundaries;
