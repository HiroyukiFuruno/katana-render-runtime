use super::super::HtmlInteractiveSession;
use super::support::{TestResult, to_string};
use crate::renderer::backends::html_browser::{
    HtmlBrowserFrame, HtmlBrowserInput, HtmlBrowserSource, HtmlBrowserViewport,
};
use crate::renderer::backends::html_interactive::types::LayoutResult;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

const HTML_SHA256: &str = "75a2a855c10c368b6aa6b5ba8f82259d882e9797cda345acc394ee3405474f61";
const SCROLL_COVERAGE_STEP_RATIO: f32 = 0.8;
const SCROLL_COVERAGE_EPSILON: f32 = 0.01;

struct Scroll {
    requested: f32,
    actual: f32,
}
struct Spec {
    path: PathBuf,
    sha: String,
    html_path: String,
    width: u32,
    height: u32,
    scale: f32,
    max_scroll_y: f32,
    scrolls: Vec<Scroll>,
}
struct Context {
    source_path: PathBuf,
    source: HtmlBrowserSource,
    html_sha: String,
    binary_sha: String,
    nonce: String,
}

#[derive(Clone, Copy)]
struct Snapshot {
    generation: u64,
    scroll_y: f32,
    content_height: f32,
    viewport: HtmlBrowserViewport,
}

struct PreparedCapture {
    frame: HtmlBrowserFrame,
    layout: LayoutResult,
    snapshot: Snapshot,
    source_before: String,
    binary_before: String,
}

struct CaptureRequest<'a> {
    context: &'a Context,
    dir: &'a Path,
    spec: &'a Spec,
    index: usize,
    scroll: &'a Scroll,
}

struct CaptureArtifacts {
    png: PathBuf,
    svg: PathBuf,
}

#[test]
fn writes_full_scroll_layout_diagnostic_when_requested() -> TestResult {
    let Some(raw_output) = std::env::var_os("KRR_HTML_FULL_SCROLL_DIAGNOSTIC_OUTPUT") else {
        return Ok(());
    };
    let output = PathBuf::from(raw_output);
    let nonce = std::env::var("KRR_HTML_FULL_SCROLL_DIAGNOSTIC_NONCE").map_err(to_string)?;
    validate_nonce(&nonce)?;
    if output.exists() {
        return Err("diagnostic output must be new".into());
    }
    fs::create_dir_all(&output).map_err(to_string)?;
    let context = diagnostic_context(nonce)?;
    let reports = manifest_paths()?
        .into_iter()
        .map(|path| capture_manifest(&context, &output, &path))
        .collect::<TestResult<Vec<_>>>()?;
    let report = json!({"htmlSha256": context.html_sha, "executionNonce": context.nonce, "binarySha256": context.binary_sha, "independentChromeAcceptance": false, "performanceAcceptance": false, "viewports": reports});
    write_json(&output.join("full-scroll-report.json"), &report)
}

fn diagnostic_context(nonce: String) -> TestResult<Context> {
    let source_path = fixture_path()?;
    let html = fs::read_to_string(&source_path).map_err(to_string)?;
    let html_sha = sha256(html.as_bytes());
    if html_sha != HTML_SHA256 {
        return Err("fixture SHA mismatch".into());
    }
    let origin = url::Url::from_file_path(&source_path).map_err(|()| "invalid origin")?;
    let source = HtmlBrowserSource::new(html, origin.as_str()).map_err(to_string)?;
    let binary = std::env::current_exe().map_err(to_string)?;
    Ok(Context {
        source_path,
        source,
        html_sha,
        binary_sha: sha256_file(&binary)?,
        nonce,
    })
}

fn write_json(path: &Path, value: &Value) -> TestResult {
    fs::write(path, serde_json::to_vec_pretty(value).map_err(to_string)?).map_err(to_string)
}

fn validate_nonce(value: &str) -> TestResult {
    if value.is_empty()
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err("invalid execution nonce".into());
    }
    Ok(())
}

fn fixture_path() -> TestResult<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/html/anonymized-requirements.html")
        .canonicalize()
        .map_err(to_string)
}

fn manifest_paths() -> TestResult<Vec<PathBuf>> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/html-scroll-coverage");
    let names = std::env::var("KRR_HTML_FULL_SCROLL_DIAGNOSTIC_MANIFESTS")
        .ok()
        .map(|s| s.split(':').map(PathBuf::from).collect())
        .unwrap_or_else(|| {
            vec![
                base.join("full-png-current-1280/scroll-coverage.json"),
                base.join("full-png-current-960/scroll-coverage.json"),
                base.join("full-png-current-620/scroll-coverage.json"),
            ]
        });
    names
        .into_iter()
        .map(|p| p.canonicalize().map_err(to_string))
        .collect()
}

fn u32_field(value: &Value, name: &str) -> TestResult<u32> {
    u32::try_from(
        value[name]
            .as_u64()
            .ok_or_else(|| format!("manifest {name} missing"))?,
    )
    .map_err(to_string)
}

fn read_spec(path: &Path) -> TestResult<Spec> {
    let bytes = fs::read(path).map_err(to_string)?;
    let value: Value = serde_json::from_slice(&bytes).map_err(to_string)?;
    if value["htmlSha256"] != HTML_SHA256 {
        return Err(format!("manifest source SHA mismatch: {}", path.display()));
    }
    let (width, height, scale, expected) = manifest_viewport(&value)?;
    let max_scroll_y = finite_nonnegative(&value["maxScrollY"], "maxScrollY")?;
    let scrolls = manifest_scrolls(&value, expected)?;
    validate_scroll_clamping(&scrolls, max_scroll_y)?;
    validate_scroll_coverage(&scrolls, max_scroll_y, height as f32)?;
    if (scrolls.last().ok_or("manifest scrolls missing")?.actual - max_scroll_y).abs() >= 0.01 {
        return Err("manifest bottom scroll mismatch".into());
    }
    Ok(Spec {
        path: path.to_owned(),
        sha: sha256(&bytes),
        html_path: value["htmlPath"]
            .as_str()
            .ok_or("manifest htmlPath missing")?
            .to_owned(),
        width,
        height,
        scale,
        max_scroll_y,
        scrolls,
    })
}

fn manifest_viewport(value: &Value) -> TestResult<(u32, u32, f32, usize)> {
    let viewport = &value["viewport"];
    let width = u32_field(viewport, "width")?;
    let height = u32_field(viewport, "height")?;
    let scale = finite_nonnegative(&viewport["scale"], "manifest scale")?;
    let expected = match (width, height, scale) {
        (1280, 900, 1.0) => 28,
        (960, 720, 1.0) => 32,
        (620, 900, 1.0) => 31,
        _ => return Err("unexpected manifest viewport".into()),
    };
    Ok((width, height, scale, expected))
}

fn manifest_scrolls(value: &Value, expected: usize) -> TestResult<Vec<Scroll>> {
    let values = value["scrolls"]
        .as_array()
        .ok_or("manifest scrolls missing")?;
    if values.len() != expected {
        return Err("manifest scroll count mismatch".into());
    }
    values
        .iter()
        .map(|v| {
            let requested = finite_nonnegative(&v["requested"], "requested")?;
            let actual = finite_nonnegative(&v["actual"], "actual")?;
            Ok(Scroll { requested, actual })
        })
        .collect()
}

fn finite_nonnegative(value: &Value, name: &str) -> TestResult<f32> {
    let number = value.as_f64().ok_or_else(|| format!("{name} missing"))?;
    let result = number as f32;
    if !number.is_finite() || !result.is_finite() || result < 0.0 {
        return Err(format!("invalid {name}"));
    }
    Ok(result)
}

fn validate_scroll_clamping(scrolls: &[Scroll], maximum: f32) -> TestResult {
    for scroll in scrolls {
        if (scroll.actual - scroll.requested.min(maximum)).abs() >= 0.01 {
            return Err("manifest scroll clamp mismatch".into());
        }
    }
    Ok(())
}

fn validate_scroll_coverage(scrolls: &[Scroll], maximum: f32, viewport_height: f32) -> TestResult {
    let mut positions: Vec<_> = scrolls.iter().map(|scroll| scroll.actual).collect();
    positions.sort_by(f32::total_cmp);
    let Some((first, remaining)) = positions.split_first() else {
        return Err("manifest scrolls missing".into());
    };
    let last = remaining.last().unwrap_or(first);
    if first.abs() >= SCROLL_COVERAGE_EPSILON || (last - maximum).abs() >= SCROLL_COVERAGE_EPSILON {
        return Err("manifest scroll coverage lacks top or bottom".into());
    }
    let gap_limit = viewport_height * SCROLL_COVERAGE_STEP_RATIO + SCROLL_COVERAGE_EPSILON;
    if positions
        .windows(2)
        .any(|pair| pair[1] - pair[0] > gap_limit)
    {
        return Err("manifest scroll coverage gap".into());
    }
    Ok(())
}

fn capture_manifest(context: &Context, output: &Path, path: &Path) -> TestResult<Value> {
    let spec = read_spec(path)?;
    if Path::new(&spec.html_path)
        .canonicalize()
        .map_err(to_string)?
        != context.source_path
    {
        return Err("manifest fixture path mismatch".into());
    }
    if sha256_file(path)? != spec.sha {
        return Err("manifest changed before capture".into());
    }
    let dir = output.join(format!("{}x{}-{}", spec.width, spec.height, context.nonce));
    fs::create_dir_all(&dir).map_err(to_string)?;
    let captures = capture_scrolls(context, &dir, &spec)?;
    if sha256_file(path)? != spec.sha {
        return Err("manifest changed after capture".into());
    }
    Ok(
        json!({"manifest": spec.path, "manifestSha256": spec.sha, "viewport": {"width": spec.width, "height": spec.height, "scale": spec.scale}, "maxScrollY": spec.max_scroll_y, "captures": captures}),
    )
}

fn capture_scrolls(context: &Context, dir: &Path, spec: &Spec) -> TestResult<Vec<Value>> {
    validate_unique_scrolls(&spec.scrolls)?;
    let viewport =
        HtmlBrowserViewport::new(spec.width, spec.height, spec.scale).map_err(to_string)?;
    let mut session =
        HtmlInteractiveSession::start(context.source.clone(), viewport).map_err(to_string)?;
    let mut captures = Vec::with_capacity(spec.scrolls.len());
    for (index, scroll) in spec.scrolls.iter().enumerate() {
        match capture_one(context, dir, spec, index, scroll, &mut session) {
            Ok(capture) => {
                captures.push(capture);
                write_partial(dir, spec, &captures, None)?;
            }
            Err(error) => {
                let message = error.to_string();
                write_partial(dir, spec, &captures, Some((index, &message)))?;
                return Err(error);
            }
        }
    }
    Ok(captures)
}

fn validate_unique_scrolls(scrolls: &[Scroll]) -> TestResult {
    let mut seen = HashSet::new();
    for scroll in scrolls {
        if !seen.insert(scroll.requested.to_bits()) {
            return Err("duplicate manifest request".into());
        }
    }
    Ok(())
}

fn capture_one(
    context: &Context,
    dir: &Path,
    spec: &Spec,
    index: usize,
    scroll: &Scroll,
    session: &mut HtmlInteractiveSession,
) -> TestResult<Value> {
    let request = CaptureRequest {
        context,
        dir,
        spec,
        index,
        scroll,
    };
    let prepared = prepare_capture(&request, session)?;
    let artifacts = write_prepared_capture(&request, &prepared)?;
    let (source_after, binary_after) = verify_provenance(
        request.context,
        &prepared.source_before,
        &prepared.binary_before,
    )?;
    capture_value(&request, prepared, artifacts, source_after, binary_after)
}

fn prepare_capture(
    request: &CaptureRequest<'_>,
    session: &mut HtmlInteractiveSession,
) -> TestResult<PreparedCapture> {
    let viewport =
        HtmlBrowserViewport::new(request.spec.width, request.spec.height, request.spec.scale)
            .map_err(to_string)?;
    let source_before = sha256_file(&request.context.source_path)?;
    let binary_before = sha256_file(&std::env::current_exe().map_err(to_string)?)?;
    if source_before != request.context.html_sha || binary_before != request.context.binary_sha {
        return Err("pre-capture provenance mismatch".into());
    }
    scroll_to_request(session, request.scroll)?;
    let frame = session
        .latest_frame()
        .ok_or("missing latest frame")?
        .clone();
    if (frame.scroll_y - request.scroll.actual).abs() >= 0.01 || frame.viewport != viewport {
        return Err("frame state mismatch".into());
    }
    let (layout, snapshot) = capture_layout(session, &frame)?;
    Ok(PreparedCapture {
        frame,
        layout,
        snapshot,
        source_before,
        binary_before,
    })
}

fn capture_layout(
    session: &mut HtmlInteractiveSession,
    frame: &HtmlBrowserFrame,
) -> TestResult<(LayoutResult, Snapshot)> {
    let snapshot = Snapshot {
        generation: frame.generation,
        scroll_y: frame.scroll_y,
        content_height: frame.content_height,
        viewport: frame.viewport,
    };
    let layout = session.layout().map_err(to_string)?;
    assert_state(session, &layout, &snapshot)?;
    Ok((layout, snapshot))
}

fn scroll_to_request(session: &mut HtmlInteractiveSession, scroll: &Scroll) -> TestResult {
    let previous = session
        .latest_frame()
        .ok_or("missing pre-scroll frame")?
        .scroll_y;
    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: scroll.requested - previous,
        })
        .map_err(to_string)
}

fn verify_provenance(
    context: &Context,
    source_before: &str,
    binary_before: &str,
) -> TestResult<(String, String)> {
    let source_after = sha256_file(&context.source_path)?;
    let binary_after = sha256_file(&std::env::current_exe().map_err(to_string)?)?;
    if source_before != source_after || binary_before != binary_after {
        return Err("capture provenance changed".into());
    }
    Ok((source_after, binary_after))
}

fn capture_value(
    request: &CaptureRequest<'_>,
    prepared: PreparedCapture,
    artifacts: CaptureArtifacts,
    source_after: String,
    binary_after: String,
) -> TestResult<Value> {
    let frame = prepared.frame;
    let snapshot = prepared.snapshot;
    Ok(
        json!({"index": request.index, "requested": request.scroll.requested, "actual": frame.scroll_y, "generation": snapshot.generation, "contentHeight": snapshot.content_height, "viewport": {"width": request.spec.width, "height": request.spec.height, "scale": request.spec.scale}, "svgPath": artifacts.svg, "svgSha256": sha256(prepared.layout.svg.as_bytes()), "pngPath": artifacts.png, "pngSha256": sha256_file(&artifacts.png)?, "sourceSha256Before": prepared.source_before, "sourceSha256After": source_after, "binarySha256Before": prepared.binary_before, "binarySha256After": binary_after, "stateGuard": true}),
    )
}

fn write_prepared_capture(
    request: &CaptureRequest<'_>,
    prepared: &PreparedCapture,
) -> TestResult<CaptureArtifacts> {
    write_capture(
        request.dir,
        request.index,
        &prepared.frame,
        &prepared.layout,
        request.spec.width,
        request.spec.height,
    )
}

fn write_capture(
    dir: &Path,
    index: usize,
    frame: &HtmlBrowserFrame,
    layout: &LayoutResult,
    width: u32,
    height: u32,
) -> TestResult<CaptureArtifacts> {
    let stem = format!("scroll-{index:03}");
    let png = dir.join(format!("{stem}.png"));
    let svg = dir.join(format!("{stem}.svg"));
    image::save_buffer_with_format(
        &png,
        &frame.pixels,
        width,
        height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(to_string)?;
    fs::write(&svg, &layout.svg).map_err(to_string)?;
    Ok(CaptureArtifacts { png, svg })
}

fn write_partial(
    dir: &Path,
    spec: &Spec,
    captures: &[Value],
    failure: Option<(usize, &str)>,
) -> TestResult {
    let mut report = json!({"manifest": spec.path, "manifestSha256": spec.sha, "viewport": {"width": spec.width, "height": spec.height, "scale": spec.scale}, "maxScrollY": spec.max_scroll_y, "captures": captures, "status": if failure.is_some() { "failed" } else { "running" }});
    if let Some((index, error)) = failure {
        report["failedIndex"] = json!(index);
        report["error"] = json!(error);
    }
    write_json(&dir.join("captures.json"), &report)
}

fn assert_state(
    session: &HtmlInteractiveSession,
    layout: &crate::renderer::backends::html_interactive::types::LayoutResult,
    state: &Snapshot,
) -> TestResult {
    let frame = session
        .latest_frame()
        .ok_or("frame disappeared after SVG")?;
    if (frame.scroll_y - state.scroll_y).abs() >= 0.01
        || (frame.content_height - state.content_height).abs() >= 0.01
        || frame.generation != state.generation
        || frame.viewport != state.viewport
        || (layout.content_height - state.content_height).abs() >= 0.01
    {
        return Err("post-SVG state mismatch".into());
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn sha256_file(path: &Path) -> TestResult<String> {
    Ok(sha256(&fs::read(path).map_err(to_string)?))
}

const COVERAGE_TEST_VIEWPORT_HEIGHT: f32 = 100.0;
const COVERAGE_TEST_MAX_SCROLL_Y: f32 = 200.0;
const COVERAGE_TEST_STEP: f32 = 80.0;
const COVERAGE_TEST_GAP_START: f32 = 60.0;
const COVERAGE_TEST_NEAR_BOTTOM: f32 = 180.0;
const COVERAGE_TEST_INTERIOR_END: f32 = 160.0;
const COVERAGE_TEST_PAST_BOTTOM_REQUEST: f32 = f32::MAX;
const COVERAGE_TEST_NONZERO_TOP: f32 = 1.0;

fn coverage_samples(samples: &[(f32, f32)]) -> Vec<Scroll> {
    samples
        .iter()
        .map(|(requested, actual)| Scroll {
            requested: *requested,
            actual: *actual,
        })
        .collect()
}

#[test]
fn full_scroll_diagnostic_fixture_sha_matches_current_pin() -> TestResult {
    let fixture = fixture_path()?;
    assert_eq!(sha256_file(&fixture)?, HTML_SHA256);
    Ok(())
}

#[test]
fn full_scroll_diagnostic_accepts_contiguous_scroll_coverage() -> TestResult {
    let samples = coverage_samples(&[
        (0.0, 0.0),
        (COVERAGE_TEST_STEP, COVERAGE_TEST_STEP),
        (COVERAGE_TEST_INTERIOR_END, COVERAGE_TEST_INTERIOR_END),
        (COVERAGE_TEST_MAX_SCROLL_Y, COVERAGE_TEST_MAX_SCROLL_Y),
    ]);
    assert!(
        validate_scroll_coverage(
            &samples,
            COVERAGE_TEST_MAX_SCROLL_Y,
            COVERAGE_TEST_VIEWPORT_HEIGHT,
        )
        .is_ok()
    );
    Ok(())
}

#[test]
fn full_scroll_diagnostic_accepts_interval_at_limit() -> TestResult {
    let at_limit = coverage_samples(&[
        (0.0, 0.0),
        (
            COVERAGE_TEST_STEP + SCROLL_COVERAGE_EPSILON,
            COVERAGE_TEST_STEP + SCROLL_COVERAGE_EPSILON,
        ),
        (COVERAGE_TEST_INTERIOR_END, COVERAGE_TEST_INTERIOR_END),
        (COVERAGE_TEST_MAX_SCROLL_Y, COVERAGE_TEST_MAX_SCROLL_Y),
    ]);
    assert!(
        validate_scroll_coverage(
            &at_limit,
            COVERAGE_TEST_MAX_SCROLL_Y,
            COVERAGE_TEST_VIEWPORT_HEIGHT,
        )
        .is_ok()
    );
    Ok(())
}

#[test]
fn full_scroll_diagnostic_accepts_repeated_actual_bottom() -> TestResult {
    let bottom = COVERAGE_TEST_MAX_SCROLL_Y;
    let samples = coverage_samples(&[
        (0.0, 0.0),
        (COVERAGE_TEST_STEP, COVERAGE_TEST_STEP),
        (COVERAGE_TEST_INTERIOR_END, COVERAGE_TEST_INTERIOR_END),
        (bottom, bottom),
        (COVERAGE_TEST_PAST_BOTTOM_REQUEST, bottom),
    ]);
    assert!(validate_scroll_coverage(&samples, bottom, COVERAGE_TEST_VIEWPORT_HEIGHT,).is_ok());
    Ok(())
}

#[test]
fn full_scroll_diagnostic_rejects_missing_scroll_coverage_bounds() -> TestResult {
    let max = COVERAGE_TEST_MAX_SCROLL_Y;
    let height = COVERAGE_TEST_VIEWPORT_HEIGHT;
    assert!(validate_scroll_coverage(&[], max, height).is_err());
    let no_top = coverage_samples(&[
        (COVERAGE_TEST_NONZERO_TOP, COVERAGE_TEST_NONZERO_TOP),
        (COVERAGE_TEST_STEP, COVERAGE_TEST_STEP),
        (COVERAGE_TEST_INTERIOR_END, COVERAGE_TEST_INTERIOR_END),
        (COVERAGE_TEST_PAST_BOTTOM_REQUEST, max),
    ]);
    assert!(validate_scroll_coverage(&no_top, max, height).is_err());
    let no_bottom = coverage_samples(&[
        (0.0, 0.0),
        (COVERAGE_TEST_STEP, COVERAGE_TEST_STEP),
        (COVERAGE_TEST_INTERIOR_END, COVERAGE_TEST_INTERIOR_END),
        (COVERAGE_TEST_NEAR_BOTTOM, COVERAGE_TEST_NEAR_BOTTOM),
    ]);
    assert!(validate_scroll_coverage(&no_bottom, max, height).is_err());
    Ok(())
}

#[test]
fn full_scroll_diagnostic_rejects_interior_scroll_coverage_gaps() -> TestResult {
    let gap = coverage_samples(&[
        (0.0, 0.0),
        (COVERAGE_TEST_GAP_START, COVERAGE_TEST_GAP_START),
        (COVERAGE_TEST_INTERIOR_END, COVERAGE_TEST_INTERIOR_END),
        (
            COVERAGE_TEST_PAST_BOTTOM_REQUEST,
            COVERAGE_TEST_MAX_SCROLL_Y,
        ),
    ]);
    assert!(
        validate_scroll_coverage(
            &gap,
            COVERAGE_TEST_MAX_SCROLL_Y,
            COVERAGE_TEST_VIEWPORT_HEIGHT,
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn full_scroll_diagnostic_rejects_invalid_nonce() {
    assert!(validate_nonce("").is_err());
    assert!(validate_nonce("bad/path").is_err());
    assert!(validate_nonce("run-20261007").is_ok());
}

#[test]
fn full_scroll_diagnostic_rejects_manifest_contract_errors() {
    let valid = json!({"viewport": {"width": 1280, "height": 900, "scale": 1.0}});
    assert!(manifest_viewport(&valid).is_ok());
    let wrong_viewport = json!({"viewport": {"width": 1279, "height": 900, "scale": 1.0}});
    assert!(manifest_viewport(&wrong_viewport).is_err());
    let overflow = json!({"viewport": {"width": u64::MAX, "height": 900, "scale": 1.0}});
    assert!(manifest_viewport(&overflow).is_err());
    let missing = json!({"scrolls": []});
    assert!(manifest_scrolls(&missing, 28).is_err());
    let invalid = json!({"scrolls": [{"requested": -1.0, "actual": 0.0}]});
    assert!(manifest_scrolls(&invalid, 1).is_err());
    let duplicate = [
        Scroll {
            requested: 0.0,
            actual: 0.0,
        },
        Scroll {
            requested: 0.0,
            actual: 0.0,
        },
    ];
    assert!(validate_unique_scrolls(&duplicate).is_err());
}

#[test]
fn full_scroll_guard_rejects_stale_real_session_snapshots() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<div style='height:500px'>state guard</div>",
        "https://example.test/guard.html",
    )
    .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(120, 80, 1.0).map_err(to_string)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;
    let frame = session.latest_frame().ok_or("missing real frame")?.clone();
    let (layout, snapshot) = capture_layout(&mut session, &frame)?;
    assert!(assert_state(&session, &layout, &snapshot).is_ok());
    assert_stale_snapshots_rejected(&session, &layout, snapshot)
}

fn assert_stale_snapshots_rejected(
    session: &HtmlInteractiveSession,
    layout: &LayoutResult,
    snapshot: Snapshot,
) -> TestResult {
    for field in 0..4 {
        let mut stale = snapshot;
        match field {
            0 => stale.generation += 1,
            1 => stale.scroll_y += 1.0,
            2 => stale.content_height += 1.0,
            _ => stale.viewport = HtmlBrowserViewport::new(121, 80, 1.0).map_err(to_string)?,
        }
        assert!(assert_state(session, layout, &stale).is_err());
    }
    Ok(())
}

#[test]
fn full_scroll_distinct_bottom_requests_remain_required_cases() -> TestResult {
    let scrolls = [
        Scroll {
            requested: 6050.0,
            actual: 6050.0,
        },
        Scroll {
            requested: 1_000_000_000.0,
            actual: 6050.0,
        },
    ];
    validate_unique_scrolls(&scrolls)?;
    validate_scroll_clamping(&scrolls, 6050.0)?;
    assert!(validate_scroll_clamping(&scrolls, 6049.0).is_err());
    Ok(())
}

#[test]
fn sequential_public_scroll_reaches_absolute_manifest_requests() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<div style='height:500px'>scroll</div>",
        "https://example.test/scroll.html",
    )
    .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(120, 80, 1.0).map_err(to_string)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;
    for requested in [160.0, 40.0, 0.0] {
        scroll_to_request(
            &mut session,
            &Scroll {
                requested,
                actual: requested,
            },
        )?;
        assert_eq!(
            session
                .latest_frame()
                .ok_or("missing scrolled frame")?
                .scroll_y,
            requested
        );
    }
    Ok(())
}
