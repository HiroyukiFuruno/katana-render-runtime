use super::super::HtmlInteractiveSession;
use super::support::{TestResult, click_element, to_string};
use crate::renderer::backends::html_browser::{HtmlBrowserSource, HtmlBrowserViewport};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf};

const MAX_JS_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[test]
fn delayed_timer_requires_host_time_advance() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<output id=state>waiting</output><script>setTimeout(() => document.querySelector('#state').textContent = 'done', 25)</script>",
        "https://example.test/timer.html",
    )
    .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(320, 240, 1.0).map_err(to_string)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;
    assert_state(&mut session, "waiting")?;
    session.advance_time(24.0).map_err(to_string)?;
    assert_state(&mut session, "waiting")?;
    session.advance_time(1.0).map_err(to_string)?;
    assert_state(&mut session, "done")?;
    Ok(())
}

#[test]
fn timed_out_event_rejects_later_timer_advance() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<button id=run style='margin:0' onclick=\"for (;;) {}\">Run</button>",
        "https://example.test/timeout.html",
    )
    .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(320, 240, 1.0).map_err(to_string)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;
    let timeout = click_element(&mut session, "run")
        .err()
        .ok_or("infinite event handler must time out")?;
    assert!(
        timeout.contains("timed out"),
        "unexpected timeout error: {timeout}"
    );
    let discarded = session
        .advance_time(1.0)
        .err()
        .ok_or("discarded runtime must reject timer advancement")?;
    assert!(
        discarded.to_string().contains("discarded"),
        "unexpected discarded runtime error: {discarded}"
    );
    Ok(())
}

fn assert_state(session: &mut HtmlInteractiveSession, expected: &str) -> TestResult {
    let actual = session
        .runtime
        .observe_feature_state("document.querySelector('#state')?.textContent")
        .map_err(to_string)?;
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn writes_fixture_feature_state_interactive() -> TestResult {
    let Some(request) = FeatureRequest::read()? else {
        return Ok(());
    };
    let resource_audit = request.operation.as_ref().map(|_| {
        crate::renderer::backends::html_subresources::resource_audit::ResourceAuditScope::start()
    });
    let (mut session, html, font_audit) = request.session()?;
    write_interactive_request(&request, &html, &mut session, font_audit, resource_audit)
}

fn write_interactive_request(
    request: &FeatureRequest,
    html: &str,
    session: &mut HtmlInteractiveSession,
    font_audit: serde_json::Value,
    resource_audit: Option<
        crate::renderer::backends::html_subresources::resource_audit::ResourceAuditScope,
    >,
) -> TestResult {
    let Some(operation) = &request.operation else {
        apply_case_operation(session, &request.case_id)?;
        return write_feature_report(request, html, session, font_audit);
    };
    validate_operation_request(operation, request)?;
    let (evidence, _pre_frame, operation_audit) =
        capture_catalog_operation(session, operation, request)?;
    write_catalog_feature_report(
        request,
        html,
        session,
        operation,
        evidence,
        merge_font_audits(font_audit, operation_audit),
        resource_audit,
    )
}

fn capture_catalog_operation(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
) -> TestResult<(serde_json::Value, serde_json::Value, serde_json::Value)> {
    let before = session
        .runtime
        .observe_feature_state(&request.expression)
        .map_err(to_string)?;
    let pre_frame = frame_metadata(session)?;
    let (evidence_result, operation_font_audit) =
        run_catalog_operation_audit(session, operation, request, before)?;
    Ok((evidence_result?, pre_frame, operation_font_audit))
}

#[cfg(target_os = "macos")]
fn run_catalog_operation_audit(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
    before: serde_json::Value,
) -> TestResult<(TestResult<serde_json::Value>, serde_json::Value)> {
    crate::markdown::svg_rasterize::SvgRasterizeOps::with_actual_font_source_audit(|| {
        apply_catalog_operation(session, operation, request, before)
    })
    .map_err(to_string)
}

#[cfg(not(target_os = "macos"))]
fn run_catalog_operation_audit(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
    before: serde_json::Value,
) -> TestResult<(TestResult<serde_json::Value>, serde_json::Value)> {
    Ok((
        apply_catalog_operation(session, operation, request, before),
        json!({"faces":[],"errors":["passive actual-source audit is available only in the macOS test runtime"],"dropped":0}),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogOperation {
    schema: String,
    #[serde(rename = "conditionId")]
    condition_id: String,
    #[serde(rename = "caseId")]
    case_id: String,
    #[serde(rename = "boundaryState")]
    boundary_state: String,
    #[serde(rename = "operationKind")]
    operation_kind: String,
    #[serde(rename = "targetSelector")]
    target_selector: Option<String>,
    viewport: CatalogViewport,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogViewport {
    width: u32,
    height: u32,
    scale: f32,
}

fn validate_operation_request(
    operation: &CatalogOperation,
    request: &FeatureRequest,
) -> TestResult {
    validate_operation_identity(operation, request)?;
    validate_operation_inventory(operation)
}

fn validate_operation_identity(
    operation: &CatalogOperation,
    request: &FeatureRequest,
) -> TestResult {
    if operation.schema != "krr-html84-operation-v1" {
        return Err("unsupported catalog operation schema".into());
    }
    let viewport = &operation.viewport;
    if viewport.width != request.width || viewport.height != request.height || viewport.scale != 1.0
    {
        return Err("catalog operation viewport differs from feature environment".into());
    }
    let expected_condition = format!(
        "{}/{}/{}x{}@{}",
        operation.case_id,
        operation.boundary_state,
        viewport.width,
        viewport.height,
        viewport.scale
    );
    if operation.condition_id != expected_condition {
        return Err(
            "catalog condition identity does not match its case, state, and viewport".into(),
        );
    }
    if operation.case_id != request.case_id {
        return Err("catalog operation case differs from legacy case environment".into());
    }
    Ok(())
}

fn validate_operation_inventory(operation: &CatalogOperation) -> TestResult {
    let viewport = &operation.viewport;
    let expected_selector = catalog_target(&operation.case_id, &operation.boundary_state);
    if operation.target_selector.as_deref() != expected_selector {
        return Err("catalog operation target differs from the finite catalog".into());
    }
    let expected_kind = catalog_operation_kind(&operation.case_id, &operation.boundary_state)
        .ok_or_else(|| "unknown catalog case or boundary state".to_owned())?;
    if operation.operation_kind != expected_kind {
        return Err("unsupported or mismatched catalog operation".into());
    }
    if ![(1280, 900), (960, 720), (620, 900)].contains(&(viewport.width, viewport.height)) {
        return Err("catalog viewport is outside the finite inventory".into());
    }
    Ok(())
}

fn catalog_target(case_id: &str, state: &str) -> Option<&'static str> {
    match (case_id, state) {
        ("typography", "ja-en-mixed" | "fractional-metrics") => Some(".mixed"),
        ("typography", "nowrap") => Some(".nowrap"),
        ("typography", "long-line") => Some("#long-line"),
        ("layout", "narrow-viewport") => Some(".flex"),
        ("layout", "multi-column") => Some(".grid"),
        ("layout", "min-content") => Some("#min-content"),
        ("table", "long-status") => Some(".badge"),
        ("table", "collapsed-border") => Some("table"),
        ("table", "rich-cell") => Some("tbody"),
        ("navigation", "top") => Some("#toc-a"),
        ("navigation", "middle") => Some("#toc-b"),
        ("navigation", "bottom") => Some("#toc-c"),
        ("navigation", "all-toc-links") => Some(".sidebar .toc-item[href^='#']"),
        ("input-state", "empty") => Some("#name"),
        ("input-state", "filled") => Some("#name"),
        ("input-state", "selected") => Some("#tab-advanced"),
        ("input-state", "expanded") => Some("#accordion"),
        ("input-state", "disabled") => Some("#disabled"),
        ("input-state", "focus-visible") => Some("#name"),
        ("decoration", "stacked") => Some(".alert"),
        ("decoration", "clipped" | "translucent") => Some(".overlay"),
        ("dynamic-dom" | "events" | "async" | "observers" | "resources", "expected-state") => None,
        _ => None,
    }
}

fn catalog_operation_kind(case_id: &str, state: &str) -> Option<&'static str> {
    Some(match (case_id, state) {
        ("navigation", "top" | "middle" | "bottom") => "navigation-pointer-click",
        ("navigation", "all-toc-links") => "navigation-all-pointer-clicks",
        ("input-state", "empty") => "input-state-empty",
        ("input-state", "filled") => "input-state-filled-save",
        ("input-state", "selected") => "input-state-selected",
        ("input-state", "expanded") => "input-state-expanded",
        ("input-state", "disabled") => "input-state-disabled-observe",
        ("input-state", "focus-visible") => "input-state-focus-visible-tab",
        ("dynamic-dom", "expected-state") => "script-load-observe",
        ("events", "expected-state") => "script-real-events",
        ("async", "expected-state") => "script-await-timer-microtask",
        ("observers", "expected-state") => "script-await-mutation-delivery",
        ("resources", "expected-state") => "script-await-resource-font",
        ("typography" | "layout" | "table" | "decoration", _) => "load-and-observe-target",
        _ => return None,
    })
}

const ASYNC_FIXTURE_SHA256: &str =
    "25fe97f94919d77d994bc193ce5b7d94ffae8d9dedd84a160061725fd43d08c7";
const ASYNC_FIXTURE_INITIAL_STATE: &str = "initial";
const ASYNC_FIXTURE_EXPECTED_STATE: &str = "microtask>ready>timer";

fn script_observation_target(case_id: &str) -> Option<&'static str> {
    match case_id {
        "dynamic-dom" => Some("#root p"),
        "events" => Some("#field"),
        "async" | "observers" | "resources" => Some("#state"),
        _ => None,
    }
}

struct FeatureRequest {
    input: PathBuf,
    source_sha256: String,
    expression: String,
    case_id: String,
    nonce: String,
    binary_sha: String,
    output: PathBuf,
    width: u32,
    height: u32,
    operation: Option<CatalogOperation>,
}

impl FeatureRequest {
    fn read() -> Result<Option<Self>, String> {
        let Some(input) = std::env::var_os("KRR_HTML_FEATURE_INPUT") else {
            return Ok(None);
        };
        let input = PathBuf::from(input).canonicalize().map_err(to_string)?;
        let source_bytes = fs::read(&input).map_err(to_string)?;
        let operation = read_operation()?;
        let expression = required_feature_env("KRR_HTML_FEATURE_EXPRESSION")?;
        let case_id = required_feature_env("KRR_HTML_FEATURE_CASE")?;
        let nonce = required_feature_env("KRR_HTML_FEATURE_NONCE")?;
        let binary_sha = required_feature_env("KRR_HTML_FEATURE_BINARY_SHA256")?;
        let output = std::env::var_os("KRR_HTML_FEATURE_OUTPUT")
            .map(PathBuf::from)
            .ok_or("missing output")?;
        let width = feature_dimension("KRR_HTML_FEATURE_WIDTH", 1280)?;
        let height = feature_dimension("KRR_HTML_FEATURE_HEIGHT", 900)?;
        Ok(Some(Self {
            input,
            source_sha256: digest(&source_bytes),
            expression,
            case_id,
            nonce,
            binary_sha,
            output,
            width,
            height,
            operation,
        }))
    }

    fn session(&self) -> TestResult<(HtmlInteractiveSession, String, serde_json::Value)> {
        let html = self.read_pinned_html()?;
        self.validate_current_binary()?;
        self.validate_requested_operation()?;
        self.start_session(html)
    }

    fn read_pinned_html(&self) -> TestResult<String> {
        let bytes = fs::read(&self.input).map_err(to_string)?;
        if digest(&bytes) != self.source_sha256 {
            return Err("catalog fixture changed after request source pin".into());
        }
        String::from_utf8(bytes).map_err(to_string)
    }

    fn validate_current_binary(&self) -> TestResult {
        let binary_path = std::env::current_exe().map_err(to_string)?;
        let binary_sha = digest(&fs::read(binary_path).map_err(to_string)?);
        if binary_sha != self.binary_sha {
            return Err("requested feature binary SHA does not match current executable".into());
        }
        Ok(())
    }

    fn validate_requested_operation(&self) -> TestResult {
        if let Some(operation) = &self.operation {
            validate_operation_request(operation, self)?;
        }
        Ok(())
    }

    fn start_session(
        &self,
        html: String,
    ) -> TestResult<(HtmlInteractiveSession, String, serde_json::Value)> {
        let origin = url::Url::from_file_path(&self.input).map_err(|()| "invalid origin")?;
        let source = HtmlBrowserSource::new(html.clone(), origin.as_str()).map_err(to_string)?;
        let viewport = HtmlBrowserViewport::new(self.width, self.height, 1.0).map_err(to_string)?;
        #[cfg(target_os = "macos")]
        let (session_result, font_audit) =
            crate::markdown::svg_rasterize::SvgRasterizeOps::with_actual_font_source_audit(|| {
                HtmlInteractiveSession::start(source, viewport)
            })
            .map_err(to_string)?;
        #[cfg(not(target_os = "macos"))]
        let (session_result, font_audit) = (
            HtmlInteractiveSession::start(source, viewport),
            json!({"faces":[],"errors":["passive actual-source audit is available only in the macOS test runtime"],"dropped":0}),
        );
        let session = session_result.map_err(to_string)?;
        Ok((session, html, font_audit))
    }
}

fn required_feature_env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(to_string)
}

fn feature_dimension(name: &str, default: u32) -> Result<u32, String> {
    match std::env::var(name) {
        Ok(value) => value.parse().map_err(to_string),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(to_string(error)),
    }
}

fn read_operation() -> Result<Option<CatalogOperation>, String> {
    match std::env::var("KRR_HTML_FEATURE_OPERATION_JSON") {
        Ok(value) => serde_json::from_str(&value).map(Some).map_err(to_string),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(to_string(error)),
    }
}

fn write_feature_report(
    request: &FeatureRequest,
    html: &str,
    session: &mut HtmlInteractiveSession,
    initial_frame_font_audit: serde_json::Value,
) -> TestResult {
    let mut report = observe_feature_report(request, html, session)?;
    attach_initial_frame_font_audit(&mut report, initial_frame_font_audit);
    validate_feature_source_pin(request)?;
    persist_feature_report(request, &report)
}

fn observe_feature_report(
    request: &FeatureRequest,
    html: &str,
    session: &mut HtmlInteractiveSession,
) -> TestResult<serde_json::Value> {
    let state = session
        .runtime
        .observe_feature_state(&request.expression)
        .map_err(to_string)?;
    let frame = session
        .latest_frame()
        .ok_or("missing post-operation frame")?;
    let capture = request.output.with_extension("png");
    let capture_sha256 = save_feature_capture(request, frame, &capture)?;
    Ok(feature_report(
        request,
        html,
        state,
        capture,
        capture_sha256,
        &session.hit_targets,
    ))
}

fn validate_feature_source_pin(request: &FeatureRequest) -> TestResult {
    if digest(&fs::read(&request.input).map_err(to_string)?) != request.source_sha256 {
        return Err("fixture source changed during feature observation".into());
    }
    Ok(())
}

fn persist_feature_report(request: &FeatureRequest, report: &serde_json::Value) -> TestResult {
    fs::write(
        &request.output,
        serde_json::to_vec_pretty(report).map_err(to_string)?,
    )
    .map_err(to_string)
}

fn attach_initial_frame_font_audit(
    report: &mut serde_json::Value,
    initial_frame_font_audit: serde_json::Value,
) {
    report["initialFrameFontAudit"] = initial_frame_font_audit;
}

fn save_feature_capture(
    request: &FeatureRequest,
    frame: &crate::renderer::backends::html_browser::HtmlBrowserFrame,
    capture: &PathBuf,
) -> TestResult<String> {
    image::save_buffer_with_format(
        capture,
        &frame.pixels,
        request.width,
        request.height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(to_string)?;
    Ok(Sha256::digest(fs::read(capture).map_err(to_string)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn feature_report(
    request: &FeatureRequest,
    html: &str,
    state: serde_json::Value,
    capture: PathBuf,
    capture_sha256: String,
    targets: &[super::super::types::HitTarget],
) -> serde_json::Value {
    let geometry = targets
        .iter()
        .map(|target| {
            json!({
                "nodeId": target.node_id,
                "x": target.x,
                "y": target.y,
                "width": target.width,
                "height": target.height,
            })
        })
        .collect::<Vec<_>>();
    json!({"htmlSha256": html_hash(html), "executionNonce": request.nonce, "binarySha256": request.binary_sha, "caseId": request.case_id, "state": state, "capturePath": capture, "captureSha256": capture_sha256, "viewport": {"width": request.width, "height": request.height, "scale": 1}, "geometry": geometry})
}

fn html_hash(html: &str) -> String {
    digest(html.as_bytes())
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn merge_font_audits(
    mut initial: serde_json::Value,
    mut operation: serde_json::Value,
) -> serde_json::Value {
    let paint_call_offset = initial
        .get("finalGlyphPaintCalls")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    offset_paint_calls(&mut operation, "finalGlyphTrees", paint_call_offset);
    offset_paint_calls(&mut operation, "finalGlyphErrors", paint_call_offset);
    merge_audit_arrays(&mut initial, &mut operation, "faces");
    merge_audit_arrays(&mut initial, &mut operation, "errors");
    merge_audit_arrays(&mut initial, &mut operation, "finalGlyphTrees");
    merge_audit_arrays(&mut initial, &mut operation, "finalGlyphErrors");
    merge_audit_arrays(&mut initial, &mut operation, "htmlFontMarkups");
    merge_audit_counters(&mut initial, &operation);
    initial
}

fn merge_audit_counters(initial: &mut serde_json::Value, operation: &serde_json::Value) {
    for key in ["dropped", "finalGlyphDropped", "htmlFontMarkupsDropped"] {
        let initial_count = initial.get(key).and_then(serde_json::Value::as_u64);
        let operation_count = operation.get(key).and_then(serde_json::Value::as_u64);
        initial[key] = match (initial_count, operation_count) {
            (Some(initial), Some(operation)) => json!(initial.saturating_add(operation)),
            _ => serde_json::Value::Null,
        };
    }
    merge_paint_call_count(initial, operation);
}

fn merge_paint_call_count(initial: &mut serde_json::Value, operation: &serde_json::Value) {
    let counts = (
        initial
            .get("finalGlyphPaintCalls")
            .and_then(serde_json::Value::as_u64),
        operation
            .get("finalGlyphPaintCalls")
            .and_then(serde_json::Value::as_u64),
    );
    initial["finalGlyphPaintCalls"] = match counts {
        (Some(initial), Some(operation)) => initial
            .checked_add(operation)
            .map_or(serde_json::Value::Null, |count| json!(count)),
        _ => serde_json::Value::Null,
    };
}

fn offset_paint_calls(audit: &mut serde_json::Value, key: &str, offset: u64) {
    let Some(rows) = audit.get_mut(key).and_then(serde_json::Value::as_array_mut) else {
        return;
    };
    for row in rows {
        if let Some(paint_call) = row.get_mut("paintCall")
            && let Some(value) = paint_call.as_u64()
        {
            *paint_call = value
                .checked_add(offset)
                .map_or(serde_json::Value::Null, |count| json!(count));
        }
    }
}

fn merge_audit_arrays(
    initial: &mut serde_json::Value,
    operation: &mut serde_json::Value,
    key: &str,
) {
    let Some(mut initial_rows) = initial
        .get(key)
        .and_then(serde_json::Value::as_array)
        .cloned()
    else {
        initial[key] = serde_json::Value::Null;
        return;
    };
    let Some(operation_rows) = operation
        .get(key)
        .and_then(serde_json::Value::as_array)
        .cloned()
    else {
        initial[key] = serde_json::Value::Null;
        return;
    };
    initial_rows.extend(operation_rows);
    initial[key] = serde_json::Value::Array(initial_rows);
}

fn audit_drop_count(audit: &serde_json::Value) -> u64 {
    audit
        .get("dropped")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

fn frame_metadata(session: &HtmlInteractiveSession) -> TestResult<serde_json::Value> {
    let frame = session.latest_frame().ok_or("missing actual html frame")?;
    Ok(
        json!({ "generation": frame.generation, "contentHeight": frame.content_height, "scrollY": frame.scroll_y }),
    )
}

fn operation_target(
    session: &mut HtmlInteractiveSession,
    selector: &str,
) -> TestResult<(u64, bool)> {
    let node = session
        .runtime
        .query_feature_node(selector)
        .ok_or_else(|| format!("catalog target not found: {selector}"))?;
    let bounds = session
        .element_boxes
        .iter()
        .find(|item| item.node_id == node.0)
        .ok_or_else(|| format!("catalog target has no actual layout box: {selector}"))?;
    let frame = session.latest_frame().ok_or("missing actual html frame")?;
    let visible = bounds.width > 0.0
        && bounds.height > 0.0
        && bounds.x < frame.viewport.logical_width()
        && bounds.x + bounds.width > 0.0
        && bounds.y < frame.scroll_y + frame.viewport.logical_height()
        && bounds.y + bounds.height > frame.scroll_y;
    Ok((node.0, visible))
}

fn pointer_click_with_trace(
    session: &mut HtmlInteractiveSession,
    node_id: u64,
) -> TestResult<Vec<serde_json::Value>> {
    let target = session
        .hit_targets
        .iter()
        .find(|target| target.node_id == node_id)
        .cloned()
        .ok_or("catalog target has no actual pointer hit target")?;
    let x = (target.x + target.width / 2.0).max(1.0);
    let y = (target.y + target.height / 2.0 - session.scroll_geometry.scroll_y).max(1.0);
    let trace = pointer_inputs(x, y);
    dispatch_pointer_trace(session, node_id, x, y, trace)
}

fn pointer_inputs(
    x: f32,
    y: f32,
) -> [(
    &'static str,
    crate::renderer::backends::html_browser::HtmlBrowserInput,
); 3] {
    use crate::renderer::backends::html_browser::HtmlBrowserInput;
    [
        ("move", HtmlBrowserInput::PointerMove { x, y }),
        ("down", HtmlBrowserInput::PointerDown { x, y, button: 0 }),
        ("up", HtmlBrowserInput::PointerUp { x, y, button: 0 }),
    ]
}

fn dispatch_pointer_trace(
    session: &mut HtmlInteractiveSession,
    node_id: u64,
    x: f32,
    y: f32,
    trace: [(
        &'static str,
        crate::renderer::backends::html_browser::HtmlBrowserInput,
    ); 3],
) -> TestResult<Vec<serde_json::Value>> {
    let mut steps = Vec::new();
    for (kind, input) in trace {
        let before = frame_metadata(session)?;
        session.dispatch_input(input).map_err(to_string)?;
        let after = frame_metadata(session)?;
        steps.push(json!({"kind": kind, "targetNodeId": node_id, "x": x, "y": y, "preFrame": before, "postFrame": after}));
    }
    Ok(steps)
}

#[derive(Default)]
struct OperationTrace {
    steps: Vec<serde_json::Value>,
    extra: serde_json::Map<String, serde_json::Value>,
}

fn apply_catalog_operation(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
    before: serde_json::Value,
) -> TestResult<serde_json::Value> {
    let start = prepare_operation_start(session, operation)?;
    let trace = run_catalog_operation(session, operation, request, start.target_id, &before)?;
    finish_catalog_operation(session, operation, request, before, start, trace)
}

struct OperationStart {
    selector: Option<String>,
    target_id: u64,
    target_visible: bool,
    pre_frame: serde_json::Value,
    hash_before: String,
}

fn prepare_operation_start(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
) -> TestResult<OperationStart> {
    let selector = operation
        .target_selector
        .as_deref()
        .or_else(|| script_observation_target(&operation.case_id));
    let target = selector
        .map(|value| operation_target(session, value))
        .transpose()?;
    let (target_id, target_visible) = target.unwrap_or((0, false));
    Ok(OperationStart {
        selector: selector.map(str::to_owned),
        target_id,
        target_visible,
        pre_frame: frame_metadata(session)?,
        hash_before: current_location_hash(session),
    })
}

fn finish_catalog_operation(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
    before: serde_json::Value,
    start: OperationStart,
    mut trace: OperationTrace,
) -> TestResult<serde_json::Value> {
    let after = session
        .runtime
        .observe_feature_state(&request.expression)
        .map_err(to_string)?;
    let post_frame = frame_metadata(session)?;
    add_navigation_result(session, operation, &start.hash_before, &mut trace.extra)?;
    validate_source_unchanged(request)?;
    build_operation_evidence(operation, request, start, before, after, post_frame, trace)
}

fn run_catalog_operation(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
    target_id: u64,
    before: &serde_json::Value,
) -> TestResult<OperationTrace> {
    match operation.operation_kind.as_str() {
        "load-and-observe-target"
        | "input-state-empty"
        | "input-state-disabled-observe"
        | "script-load-observe" => Ok(OperationTrace::default()),
        "input-state-filled-save"
        | "input-state-selected"
        | "input-state-expanded"
        | "input-state-focus-visible-tab"
        | "navigation-pointer-click" => run_visual_operation(session, operation, target_id),
        "navigation-all-pointer-clicks" => run_all_toc_operation(session, operation, request),
        "script-real-events"
        | "script-await-timer-microtask"
        | "script-await-mutation-delivery"
        | "script-await-resource-font" => {
            run_script_operation(session, operation, request, target_id, before)
        }
        _ => Err(format!(
            "unknown catalog operation: {}",
            operation.operation_kind
        )),
    }
}

fn run_visual_operation(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    target_id: u64,
) -> TestResult<OperationTrace> {
    let mut trace = OperationTrace::default();
    match operation.operation_kind.as_str() {
        "input-state-filled-save" => trace.steps = apply_filled_save(session)?,
        "input-state-selected" | "input-state-expanded" | "navigation-pointer-click" => {
            trace.steps = pointer_click_with_trace(session, target_id)?;
        }
        "input-state-focus-visible-tab" => apply_tab_observation(session, target_id, &mut trace)?,
        _ => return Err("unsupported visual catalog operation".into()),
    }
    Ok(trace)
}

fn run_all_toc_operation(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
) -> TestResult<OperationTrace> {
    let mut trace = OperationTrace::default();
    apply_all_toc_links(session, operation, request, &mut trace)?;
    Ok(trace)
}

fn run_script_operation(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
    target_id: u64,
    before: &serde_json::Value,
) -> TestResult<OperationTrace> {
    let mut trace = OperationTrace::default();
    match operation.operation_kind.as_str() {
        "script-real-events" => apply_script_events(session, request, target_id, &mut trace)?,
        "script-await-timer-microtask" => {
            apply_timer_observation(session, operation, request, before, &mut trace)?
        }
        "script-await-mutation-delivery" => {
            apply_mutation_observation(session, request, before, &mut trace)?
        }
        "script-await-resource-font" => {
            apply_resource_settled_observation(session, operation, request, before, &mut trace)?;
        }
        _ => return Err("unsupported script catalog operation".into()),
    }
    Ok(trace)
}

fn apply_filled_save(session: &mut HtmlInteractiveSession) -> TestResult<Vec<serde_json::Value>> {
    let mut steps = Vec::new();
    let input_id = operation_target(session, "#name")?.0;
    steps.extend(pointer_click_with_trace(session, input_id)?);
    let value_before = observe_input_value(session)?;
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::Text {
                text: "架空の名称".into(),
            },
        )
        .map_err(to_string)?;
    let value_after = observe_input_value(session)?;
    steps.push(json!({"kind":"text","targetNodeId":input_id,"text":"架空の名称","before":value_before,"after":value_after}));
    let save_id = operation_target(session, "#save")?.0;
    steps.extend(pointer_click_with_trace(session, save_id)?);
    Ok(steps)
}

fn observe_input_value(session: &mut HtmlInteractiveSession) -> TestResult<serde_json::Value> {
    session
        .runtime
        .observe_feature_state("document.querySelector('#name').value")
        .map_err(to_string)
}

fn apply_tab_observation(
    session: &mut HtmlInteractiveSession,
    target_id: u64,
    trace: &mut OperationTrace,
) -> TestResult {
    let focused_before = session.focused_node;
    let active_before = observe_active_element_id(session)?;
    let focus_visible_before = session.focus_visible;
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::KeyDown {
                key: "Tab".into(),
            },
        )
        .map_err(to_string)?;
    let focused_after = session.focused_node;
    let active_after = observe_active_element_id(session)?;
    let target_focused = focused_after == Some(target_id);
    let active_target_matches = active_after.as_str() == Some("name");
    let focus_visible_after = session.focus_visible;
    let focus_visible = focus_visible_after && focused_after.is_some();
    trace.steps.push(json!({"kind":"key","key":"Tab","targetNodeId":target_id,"focusedNodeBefore":focused_before,"focusedNodeAfter":focused_after,"activeElementBefore":active_before,"activeElementAfter":active_after,"focusVisibleBefore":focus_visible_before,"focusVisibleAfter":focus_visible_after,"targetFocused":target_focused,"activeElementMatchesTarget":active_target_matches,"focusVisible":focus_visible}));
    if !target_focused || !active_target_matches || !focus_visible {
        mark_operation_unknown(
            trace,
            "Tab did not focus #name with matching activeElement and focus-visible state",
        );
    }
    Ok(())
}

fn observe_active_element_id(
    session: &mut HtmlInteractiveSession,
) -> TestResult<serde_json::Value> {
    session
        .runtime
        .observe_feature_state("document.activeElement.id")
        .map_err(to_string)
}

fn apply_all_toc_links(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
    trace: &mut OperationTrace,
) -> TestResult {
    let links = super::toc_diagnostic::capture_catalog_all_toc_links(
        session,
        &request.output.with_extension("toc"),
        request.width,
        request.height,
        operation.viewport.scale,
    )?;
    for link in &links {
        if let Some(pointer) = link
            .get("pointerTrace")
            .and_then(serde_json::Value::as_array)
        {
            trace.steps.extend(pointer.iter().cloned());
        }
    }
    trace.extra.insert("links".into(), json!(links));
    Ok(())
}

fn apply_script_events(
    session: &mut HtmlInteractiveSession,
    request: &FeatureRequest,
    field_id: u64,
    trace: &mut OperationTrace,
) -> TestResult {
    let before_focus = observe_event_state(session, request)?;
    trace
        .steps
        .extend(pointer_click_with_trace(session, field_id)?);
    let focused = session.focused_input == Some(field_id);
    let after_focus = observe_event_state(session, request)?;
    let mut events = vec![
        json!({"type":"focus","targetNodeId":field_id,"focused":focused,"stateBefore":before_focus,"stateAfter":after_focus}),
    ];
    append_input_event(session, request, field_id, trace, &mut events)?;
    append_key_event(session, request, field_id, trace, &mut events)?;
    append_submit_events(session, request, trace, &mut events)?;
    trace.extra.insert("events".into(), json!(events));
    Ok(())
}

fn observe_event_state(
    session: &mut HtmlInteractiveSession,
    request: &FeatureRequest,
) -> TestResult<serde_json::Value> {
    session
        .runtime
        .observe_feature_state(&request.expression)
        .map_err(to_string)
}

fn append_input_event(
    session: &mut HtmlInteractiveSession,
    request: &FeatureRequest,
    field_id: u64,
    trace: &mut OperationTrace,
    events: &mut Vec<serde_json::Value>,
) -> TestResult {
    let before = observe_event_state(session, request)?;
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::Text {
                text: "入力".into(),
            },
        )
        .map_err(to_string)?;
    let after = observe_event_state(session, request)?;
    trace.steps.push(json!({
        "kind":"text", "targetNodeId":field_id, "text":"入力",
        "stateBefore":before, "stateAfter":after
    }));
    events.push(
        json!({"type":"input","targetNodeId":field_id,"stateBefore":before,"stateAfter":after}),
    );
    Ok(())
}

fn append_key_event(
    session: &mut HtmlInteractiveSession,
    request: &FeatureRequest,
    field_id: u64,
    trace: &mut OperationTrace,
    events: &mut Vec<serde_json::Value>,
) -> TestResult {
    append_key_down_event(session, request, field_id, trace, events)?;
    append_key_up_event(session, request, field_id, trace, events)
}

fn append_key_down_event(
    session: &mut HtmlInteractiveSession,
    request: &FeatureRequest,
    field_id: u64,
    trace: &mut OperationTrace,
    events: &mut Vec<serde_json::Value>,
) -> TestResult {
    let before = observe_event_state(session, request)?;
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::KeyDown {
                key: "Shift".into(),
            },
        )
        .map_err(to_string)?;
    let after = observe_event_state(session, request)?;
    trace.steps.push(json!({
        "kind":"key_down", "targetNodeId":field_id, "key":"Shift",
        "stateBefore":before, "stateAfter":after
    }));
    events.push(json!({"type":"keydown","targetNodeId":field_id,"key":"Shift","stateBefore":before,"stateAfter":after}));
    Ok(())
}

fn append_key_up_event(
    session: &mut HtmlInteractiveSession,
    request: &FeatureRequest,
    field_id: u64,
    trace: &mut OperationTrace,
    events: &mut Vec<serde_json::Value>,
) -> TestResult {
    let before_key_up = observe_event_state(session, request)?;
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::KeyUp {
                key: "Shift".into(),
            },
        )
        .map_err(to_string)?;
    let after_key_up = observe_event_state(session, request)?;
    trace.steps.push(json!({
        "kind":"key_up", "targetNodeId":field_id, "key":"Shift",
        "stateBefore":before_key_up, "stateAfter":after_key_up
    }));
    events.push(json!({
        "type":"keyup", "targetNodeId":field_id, "key":"Shift",
        "stateBefore":before_key_up, "stateAfter":after_key_up
    }));
    Ok(())
}

fn append_submit_events(
    session: &mut HtmlInteractiveSession,
    request: &FeatureRequest,
    trace: &mut OperationTrace,
    events: &mut Vec<serde_json::Value>,
) -> TestResult {
    let button_id = session
        .runtime
        .query_feature_node("button")
        .ok_or("script event button target is missing")?
        .0;
    let form_id = session
        .runtime
        .query_feature_node("form")
        .ok_or("script event form target is missing")?
        .0;
    let before = observe_event_state(session, request)?;
    trace
        .steps
        .extend(pointer_click_with_trace(session, button_id)?);
    let after = observe_event_state(session, request)?;
    events.push(
        json!({"type":"click","targetNodeId":button_id,"stateBefore":before,"stateAfter":after}),
    );
    events.push(
        json!({"type":"submit","targetNodeId":form_id,"stateBefore":before,"stateAfter":after}),
    );
    Ok(())
}

fn apply_timer_observation(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
    before: &serde_json::Value,
    trace: &mut OperationTrace,
) -> TestResult {
    let expected = expected_async_timer_state(session, operation, request)?;
    let timer_ran = session.runtime.advance_timers(0.0).map_err(to_string)?;
    if timer_ran {
        session.refresh_frame().map_err(to_string)?;
    }
    let after = observe_event_state(session, request)?;
    let verified = expected
        .as_ref()
        .is_some_and(|expected| async_timer_state_matches(timer_ran, before, &after, expected));
    if !verified {
        mark_operation_unknown(
            trace,
            "Timer delivery did not match the pinned async fixture state transition",
        );
    }
    record_timer_delivery(trace, timer_ran, before, after, verified);
    Ok(())
}

fn record_timer_delivery(
    trace: &mut OperationTrace,
    timer_ran: bool,
    before: &serde_json::Value,
    after: serde_json::Value,
    verified: bool,
) {
    let delivery = if timer_ran {
        "timer-and-microtask"
    } else {
        "session-load-settled"
    };
    trace.extra.insert("delivery".into(), json!(delivery));
    trace.steps.push(json!({"kind":"host-time-advance","elapsedMs":0.0,"timerRan":timer_ran,"microtaskCheckpoint":"runtime-advance-timers","stateBefore":before,"stateAfter":after,"fixtureStateVerified":verified}));
}

fn expected_async_timer_state(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
) -> TestResult<Option<serde_json::Value>> {
    if !is_pinned_async_timer_operation(operation, request)? {
        return Ok(None);
    }
    let expected = session
        .runtime
        .observe_feature_state("JSON.parse(document.querySelector('#expected-state').textContent)")
        .map_err(to_string)?;
    Ok((expected == json!(ASYNC_FIXTURE_EXPECTED_STATE)).then_some(expected))
}

fn is_pinned_async_timer_operation(
    operation: &CatalogOperation,
    request: &FeatureRequest,
) -> TestResult<bool> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/html/aspects/script-async.html")
        .canonicalize()
        .map_err(to_string)?;
    Ok(operation.schema == "krr-html84-operation-v1"
        && operation.case_id == "async"
        && operation.boundary_state == "expected-state"
        && operation.operation_kind == "script-await-timer-microtask"
        && operation.target_selector.is_none()
        && [(1280, 900), (960, 720), (620, 900)]
            .contains(&(operation.viewport.width, operation.viewport.height))
        && operation.viewport.scale == 1.0
        && operation.condition_id
            == format!(
                "async/expected-state/{}x{}@{}",
                operation.viewport.width, operation.viewport.height, operation.viewport.scale
            )
        && request.case_id == "async"
        && request.input == fixture
        && request.source_sha256 == ASYNC_FIXTURE_SHA256)
}

fn async_timer_state_matches(
    timer_ran: bool,
    before: &serde_json::Value,
    after: &serde_json::Value,
    expected: &serde_json::Value,
) -> bool {
    if timer_ran {
        before.as_str() == Some(ASYNC_FIXTURE_INITIAL_STATE) && after == expected
    } else {
        before == expected && after == expected
    }
}

fn apply_mutation_observation(
    session: &mut HtmlInteractiveSession,
    request: &FeatureRequest,
    before: &serde_json::Value,
    trace: &mut OperationTrace,
) -> TestResult {
    let settled = observe_event_state(session, request)?;
    record_mutation_observation(trace, before, &settled);
    Ok(())
}

fn record_mutation_observation(
    trace: &mut OperationTrace,
    before: &serde_json::Value,
    settled: &serde_json::Value,
) {
    let state_text = before
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| before.to_string());
    let verified = ["mutation", "resize", "intersection"]
        .iter()
        .all(|event| state_text.contains(event));
    if verified {
        trace
            .extra
            .insert("delivery".into(), json!("session-load-settled"));
        trace.steps.push(json!({"kind":"session-load-settled-observer-state","stateBefore":before,"stateAfter":settled}));
    } else {
        trace
            .extra
            .insert("delivery".into(), json!("session-load-observation"));
        trace.steps.push(json!({"kind":"session-load-observer-state-observation","stateBefore":before,"stateAfter":settled,"fixtureStateVerified":false}));
        mark_operation_unknown(
            trace,
            "The settled session-load state does not expose the mutation, resize, and intersection delivery records",
        );
    }
}

fn apply_resource_settled_observation(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
    before: &serde_json::Value,
    trace: &mut OperationTrace,
) -> TestResult {
    let expected = expected_resource_state(session, operation, request)?;
    let after = observe_event_state(session, request)?;
    record_resource_observation(trace, before, &after, expected.as_ref());
    Ok(())
}

fn record_resource_observation(
    trace: &mut OperationTrace,
    before: &serde_json::Value,
    after: &serde_json::Value,
    expected: Option<&serde_json::Value>,
) {
    let Some(expected) = expected else {
        record_unknown_resource_state(trace, before, after, None);
        mark_operation_unknown(
            trace,
            "Resource settlement is not bound to the catalog fixture",
        );
        return;
    };
    if resource_state_matches_fixture(before, after, expected) {
        trace
            .extra
            .insert("delivery".into(), json!("session-load-settled"));
        trace.steps.push(json!({"kind":"session-load-settled-resource-state","stateBefore":before,"stateAfter":after}));
    } else {
        record_unknown_resource_state(trace, before, after, Some(expected));
        mark_operation_unknown(
            trace,
            "Resource and font-ready state did not settle to the pinned fixture state",
        );
    }
}

fn record_unknown_resource_state(
    trace: &mut OperationTrace,
    before: &serde_json::Value,
    after: &serde_json::Value,
    expected: Option<&serde_json::Value>,
) {
    trace
        .extra
        .insert("delivery".into(), json!("session-load-observation"));
    trace.steps.push(json!({"kind":"session-load-resource-state-observation","stateBefore":before,"stateAfter":after,"expectedState":expected,"fixtureStateVerified":false}));
}

fn expected_resource_state(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    request: &FeatureRequest,
) -> TestResult<Option<serde_json::Value>> {
    if operation.case_id != "resources"
        || operation.boundary_state != "expected-state"
        || operation.operation_kind != "script-await-resource-font"
        || request.case_id != "resources"
        || !is_pinned_resource_fixture(request)?
    {
        return Ok(None);
    }
    session
        .runtime
        .observe_feature_state("JSON.parse(document.querySelector('#expected-state').textContent)")
        .map(Some)
        .map_err(to_string)
}

fn is_pinned_resource_fixture(request: &FeatureRequest) -> TestResult<bool> {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/html/aspects/script-resources.html")
        .canonicalize()
        .map_err(to_string)?;
    Ok(request.input == fixture)
}

fn resource_state_matches_fixture(
    before: &serde_json::Value,
    after: &serde_json::Value,
    expected: &serde_json::Value,
) -> bool {
    before == expected && after == expected
}

fn mark_operation_unknown(trace: &mut OperationTrace, reason: &str) {
    trace.extra.insert("status".into(), json!("unknown"));
    trace.extra.insert("unknownReason".into(), json!(reason));
}

fn add_navigation_result(
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    hash_before: &str,
    extra: &mut serde_json::Map<String, serde_json::Value>,
) -> TestResult {
    if operation.operation_kind == "navigation-pointer-click" {
        add_single_navigation_result(session, hash_before, extra)?;
    }
    Ok(())
}

fn add_single_navigation_result(
    session: &mut HtmlInteractiveSession,
    hash_before: &str,
    extra: &mut serde_json::Map<String, serde_json::Value>,
) -> TestResult {
    let hash_after = current_location_hash(session);
    let destination_id = hash_after
        .strip_prefix('#')
        .and_then(|id| session.runtime.node_for_element_id(id))
        .map(|id| id.0)
        .ok_or("navigation destination target is missing")?;
    let destination = session
        .element_boxes
        .iter()
        .find(|item| item.node_id == destination_id)
        .ok_or("navigation destination has no actual layout box")?;
    let frame = session
        .latest_frame()
        .ok_or("navigation destination frame is missing")?;
    let visible = destination.width > 0.0
        && destination.height > 0.0
        && destination.y < frame.scroll_y + frame.viewport.logical_height()
        && destination.y + destination.height > frame.scroll_y;
    extra.insert("hashBefore".into(), json!(hash_before));
    extra.insert("hashAfter".into(), json!(hash_after));
    extra.insert("destination".into(), json!(hash_after));
    extra.insert("destinationTarget".into(), json!({"selector":format!("#{}",hash_after.trim_start_matches('#')),"nodeId":destination_id,"visible":visible,"scrollY":frame.scroll_y}));
    Ok(())
}

fn validate_source_unchanged(request: &FeatureRequest) -> TestResult {
    let source_hash_after = digest(&fs::read(&request.input).map_err(to_string)?);
    if request.source_sha256 != source_hash_after {
        return Err("fixture source changed during catalog observation".into());
    }
    Ok(())
}

fn build_operation_evidence(
    operation: &CatalogOperation,
    request: &FeatureRequest,
    start: OperationStart,
    before: serde_json::Value,
    after: serde_json::Value,
    post_frame: serde_json::Value,
    mut trace: OperationTrace,
) -> TestResult<serde_json::Value> {
    let mut evidence = json!({
        "schema":"krr-html84-operation-evidence-v1", "kind":operation.operation_kind,
        "sourceSha256Before":request.source_sha256, "sourceSha256After":request.source_sha256,
        "preFrame":start.pre_frame, "postFrame":post_frame,
        "target":{"selector":start.selector.as_deref(), "nodeId":start.target_id, "visible":start.target_visible},
        "state":{"before":before,"after":after}, "steps":trace.steps,
        "delivery":"session-load"
    });
    let object = evidence
        .as_object_mut()
        .ok_or("invalid operation evidence object")?;
    object.extend(std::mem::take(&mut trace.extra));
    set_operation_evidence_status(object);
    Ok(evidence)
}

fn set_operation_evidence_status(object: &mut serde_json::Map<String, serde_json::Value>) {
    if object.get("status").and_then(serde_json::Value::as_str) == Some("unknown") {
        let reason = object
            .get("unknownReason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("operation is unsupported")
            .to_owned();
        object.insert("reason".into(), json!(reason));
    } else {
        object.insert("status".into(), json!("observed"));
    }
}

fn current_location_hash(session: &HtmlInteractiveSession) -> String {
    match session.source.origin.url().fragment() {
        Some(fragment) => format!("#{fragment}"),
        None => String::new(),
    }
}

struct CatalogCapture {
    state: serde_json::Value,
    frame: serde_json::Value,
    capture: PathBuf,
    capture_sha256: String,
    geometry: serde_json::Value,
    binary_sha256: String,
}

fn write_catalog_feature_report(
    request: &FeatureRequest,
    html: &str,
    session: &mut HtmlInteractiveSession,
    operation: &CatalogOperation,
    evidence: serde_json::Value,
    font_audit: serde_json::Value,
    resource_audit: Option<
        crate::renderer::backends::html_subresources::resource_audit::ResourceAuditScope,
    >,
) -> TestResult {
    let capture = collect_catalog_capture(request, session, &evidence)?;
    let resource_inventory = resource_audit
        .ok_or("catalog resource audit scope is missing")?
        .finish(&request.output.with_extension("resource-bodies"))?;
    let details = catalog_report_details(
        &evidence,
        operation,
        request,
        font_audit,
        resource_inventory,
    );
    let report = build_catalog_report(request, html, operation, capture, details);
    write_catalog_report(request, report)
}

fn catalog_report_details(
    evidence: &serde_json::Value,
    operation: &CatalogOperation,
    request: &FeatureRequest,
    font_audit: serde_json::Value,
    resource_inventory: crate::renderer::backends::html_subresources::resource_audit::Inventory,
) -> CatalogReportEvidence {
    let fonts = catalog_font_provenance(&font_audit);
    let observed = observed_operation_evidence(evidence, operation, request);
    let diagnostic = diagnostic_operation_evidence(evidence, operation, request);
    let font_audit_complete = catalog_font_audit_complete(&font_audit, &fonts);
    let reason = catalog_unknown_reason(evidence, &font_audit, &fonts, &resource_inventory);
    CatalogReportEvidence {
        fonts,
        resource_inventory,
        font_audit_complete,
        reason,
        observed,
        diagnostic,
        target_visible: evidence["target"]["visible"].clone(),
    }
}

fn collect_catalog_capture(
    request: &FeatureRequest,
    session: &mut HtmlInteractiveSession,
    evidence: &serde_json::Value,
) -> TestResult<CatalogCapture> {
    let (state, capture, capture_sha256) = capture_catalog_frame(request, session)?;
    let node_id = evidence["target"]["nodeId"]
        .as_u64()
        .ok_or("catalog target evidence is missing a numeric nodeId")?;
    let geometry = catalog_target_geometry(session, node_id);
    let binary_sha256 = current_feature_binary_sha()?;
    if binary_sha256 != request.binary_sha {
        return Err("current feature executable changed during observation".into());
    }
    Ok(CatalogCapture {
        state,
        frame: frame_metadata(session)?,
        capture,
        capture_sha256,
        geometry,
        binary_sha256,
    })
}

fn capture_catalog_frame(
    request: &FeatureRequest,
    session: &mut HtmlInteractiveSession,
) -> TestResult<(serde_json::Value, PathBuf, String)> {
    let state = session
        .runtime
        .observe_feature_state(&request.expression)
        .map_err(to_string)?;
    let frame = session
        .latest_frame()
        .ok_or("missing post-operation frame")?;
    let capture = request.output.with_extension("png");
    let capture_sha256 = save_feature_capture(request, frame, &capture)?;
    Ok((state, capture, capture_sha256))
}

fn current_feature_binary_sha() -> TestResult<String> {
    let binary_path = std::env::current_exe().map_err(to_string)?;
    Ok(digest(&fs::read(binary_path).map_err(to_string)?))
}

fn catalog_target_geometry(session: &HtmlInteractiveSession, node_id: u64) -> serde_json::Value {
    let boxes = session.element_boxes.iter().filter(|item| item.node_id == node_id)
        .map(|item| json!({"nodeId":item.node_id,"x":item.x,"y":item.y,"width":item.width,"height":item.height}))
        .collect::<Vec<_>>();
    json!(boxes)
}

fn catalog_font_provenance(font_audit: &serde_json::Value) -> serde_json::Value {
    let Some(records) = final_frame_font_records(font_audit) else {
        return json!([]);
    };
    json!(records)
}

fn final_frame_font_records(font_audit: &serde_json::Value) -> Option<Vec<serde_json::Value>> {
    let trees = font_audit.get("finalGlyphTrees")?.as_array()?;
    let selected_trees = final_paint_trees(font_audit, trees)?;
    collect_final_font_records(selected_trees)
}

fn final_paint_trees<'a>(
    audit: &serde_json::Value,
    trees: &'a [serde_json::Value],
) -> Option<Vec<&'a serde_json::Value>> {
    let paint_call = latest_root_paint_call(trees)?;
    if !final_paint_audit_is_complete(audit, paint_call)? {
        return None;
    }
    let selected = trees
        .iter()
        .filter(|tree| {
            tree.get("paintCall").and_then(serde_json::Value::as_u64) == Some(paint_call)
        })
        .collect::<Vec<_>>();
    valid_final_tree_paths(&selected).then_some(selected)
}

fn latest_root_paint_call(trees: &[serde_json::Value]) -> Option<u64> {
    trees
        .iter()
        .filter(|tree| tree.get("treePath").and_then(serde_json::Value::as_str) == Some("root"))
        .filter_map(|tree| tree.get("paintCall").and_then(serde_json::Value::as_u64))
        .max()
}

fn final_paint_audit_is_complete(audit: &serde_json::Value, paint_call: u64) -> Option<bool> {
    Some(
        audit
            .get("finalGlyphPaintCalls")
            .and_then(serde_json::Value::as_u64)
            == Some(paint_call)
            && audit.get("finalGlyphErrors")?.as_array()?.is_empty()
            && audit
                .get("finalGlyphDropped")
                .and_then(serde_json::Value::as_u64)
                == Some(0),
    )
}

fn valid_final_tree_paths(trees: &[&serde_json::Value]) -> bool {
    trees
        .iter()
        .filter(|tree| tree_path(tree) == Some("root"))
        .count()
        == 1
        && trees.iter().all(|tree| {
            tree_path(tree).is_some_and(|path| path == "root" || path.starts_with("root/"))
        })
}

fn tree_path(tree: &serde_json::Value) -> Option<&str> {
    tree.get("treePath").and_then(serde_json::Value::as_str)
}

fn collect_final_font_records(trees: Vec<&serde_json::Value>) -> Option<Vec<serde_json::Value>> {
    let mut records = BTreeMap::new();
    for tree in trees {
        for face in tree.get("faces")?.as_array()? {
            collect_verified_face(&mut records, face, tree)?;
        }
    }
    (!records.is_empty()).then(|| records.into_values().collect())
}

type CatalogFontRecords = BTreeMap<(String, u64, String), serde_json::Value>;

fn collect_verified_face(
    records: &mut CatalogFontRecords,
    face: &serde_json::Value,
    tree: &serde_json::Value,
) -> Option<()> {
    if face.get("identity").and_then(serde_json::Value::as_str) != Some("physical_verified")
        || face.get("glyphCount").and_then(serde_json::Value::as_u64)? == 0
    {
        return None;
    }
    let layout_runs = face
        .get("faceId")
        .and_then(serde_json::Value::as_str)
        .map_or(Some(Vec::new()), |face_id| {
            layout_runs_for_face(tree, face_id)
        })?;
    let record = catalog_font_record(face.get("source")?, layout_runs)?;
    let key = (
        record["filePath"].as_str()?.to_owned(),
        record["faceIndex"].as_u64()?,
        record["fileSha256"].as_str()?.to_owned(),
    );
    merge_catalog_font_record(records, key, record)
}

fn merge_catalog_font_record(
    records: &mut CatalogFontRecords,
    key: (String, u64, String),
    record: serde_json::Value,
) -> Option<()> {
    match records.get_mut(&key) {
        Some(existing) if existing == &record => Some(()),
        Some(existing) if same_catalog_font_identity(existing, &record) => {
            merge_layout_runs(existing, &record)?;
            Some(())
        }
        None => {
            records.insert(key, record);
            Some(())
        }
        _ => None,
    }
}

fn catalog_font_record(
    face: &serde_json::Value,
    layout_runs: Vec<serde_json::Value>,
) -> Option<serde_json::Value> {
    let family = face.get("families")?.as_array()?.first()?.as_str()?;
    let source = face.get("kind")?.as_str()?;
    let file_path = face.get("path")?.as_str()?;
    let face_index = face.get("index")?.as_u64()?;
    let file_sha = face.get("bytes_sha256")?.as_str()?;
    if family.is_empty()
        || source.is_empty()
        || !std::path::Path::new(file_path).is_absolute()
        || !is_sha256(file_sha)
        || face_index > MAX_JS_SAFE_INTEGER
    {
        return None;
    }
    let mut record = json!({
        "family":family, "source":source, "filePath":file_path, "faceIndex":face_index,
        "fileSha256":file_sha, "scope":"capture", "selected":true, "fileBytesVerified":true
    });
    if !layout_runs.is_empty() {
        record["layoutRuns"] = serde_json::Value::Array(layout_runs);
    }
    Some(record)
}

fn layout_runs_for_face(tree: &serde_json::Value, face_id: &str) -> Option<Vec<serde_json::Value>> {
    let mut runs = Vec::new();
    let Some(spans) = tree.get("textSpans").and_then(serde_json::Value::as_array) else {
        return Some(runs);
    };
    for span in spans {
        if let Some(run) = layout_run_for_face(tree, span, face_id)? {
            runs.push(run);
        }
    }
    Some(runs)
}

fn layout_run_for_face(
    tree: &serde_json::Value,
    span: &serde_json::Value,
    face_id: &str,
) -> Option<Option<serde_json::Value>> {
    let span_face = span_face(span, face_id);
    let Some(span_face) = span_face else {
        return Some(None);
    };
    let glyph_count = span_face.get("glyphCount")?.as_u64()?;
    if glyph_count == 0 {
        return None;
    }
    Some(Some(layout_run_value(
        tree,
        span,
        span_face,
        face_id,
        glyph_count,
    )?))
}

fn span_face<'a>(span: &'a serde_json::Value, face_id: &str) -> Option<&'a serde_json::Value> {
    span.get("faces")?
        .as_array()?
        .iter()
        .find(|face| face.get("faceId").and_then(serde_json::Value::as_str) == Some(face_id))
}

fn layout_run_value(
    tree: &serde_json::Value,
    span: &serde_json::Value,
    span_face: &serde_json::Value,
    face_id: &str,
    glyph_count: u64,
) -> Option<serde_json::Value> {
    Some(json!({
        "paintCall": tree.get("paintCall").and_then(serde_json::Value::as_u64)?,
        "treePath": tree.get("treePath").and_then(serde_json::Value::as_str)?,
        "faceId": face_id, "glyphCount": glyph_count,
        "layoutVariations": span.get("layoutVariations")?.as_array()?.clone(),
        "derivedEffectiveVariations": span_face.get("effectiveVariations")?.as_array()?.clone(),
        "derivedAppliedDesignCoordinates": span_face
            .get("derivedAppliedDesignCoordinates")?
            .as_array()?
            .clone(),
        "derivedNormalizedCoords": span_face.get("normalizedCoords")?.as_array()?.clone(),
    }))
}

fn same_catalog_font_identity(left: &serde_json::Value, right: &serde_json::Value) -> bool {
    [
        "family",
        "source",
        "filePath",
        "faceIndex",
        "fileSha256",
        "scope",
        "selected",
        "fileBytesVerified",
    ]
    .iter()
    .all(|key| left.get(*key) == right.get(*key))
}

fn merge_layout_runs(existing: &mut serde_json::Value, incoming: &serde_json::Value) -> Option<()> {
    let incoming_runs = incoming.get("layoutRuns")?.as_array()?;
    let existing_runs = existing
        .as_object_mut()?
        .entry("layoutRuns")
        .or_insert_with(|| serde_json::Value::Array(Vec::new()))
        .as_array_mut()?;
    existing_runs.extend(incoming_runs.iter().cloned());
    Some(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn observed_operation_evidence(
    evidence: &serde_json::Value,
    operation: &CatalogOperation,
    request: &FeatureRequest,
) -> Option<serde_json::Value> {
    if !operation_evidence_matches_request(evidence, operation, request)? {
        return None;
    }
    let mut observed = evidence.clone();
    if let Some(object) = observed.as_object_mut() {
        object.remove("status");
        object.remove("unknownReason");
        object.remove("reason");
    }
    Some(observed)
}

fn diagnostic_operation_evidence(
    evidence: &serde_json::Value,
    operation: &CatalogOperation,
    request: &FeatureRequest,
) -> Option<serde_json::Value> {
    let diagnostic_kind = matches!(
        operation.operation_kind.as_str(),
        "script-await-timer-microtask"
            | "script-await-mutation-delivery"
            | "script-await-resource-font"
    );
    if !diagnostic_kind
        || evidence.get("status")?.as_str() != Some("unknown")
        || !operation_evidence_identity_matches_request(evidence, operation, request)?
    {
        return None;
    }
    Some(evidence.clone())
}

fn operation_evidence_matches_request(
    evidence: &serde_json::Value,
    operation: &CatalogOperation,
    request: &FeatureRequest,
) -> Option<bool> {
    if evidence.get("status")?.as_str() != Some("observed")
        || !operation_evidence_identity_matches_request(evidence, operation, request)?
    {
        return Some(false);
    }
    let target = evidence.get("target")?;
    let pre_frame = evidence.get("preFrame")?;
    let post_frame = evidence.get("postFrame")?;
    let state = evidence.get("state")?;
    Some(
        target["visible"] == true
            && target["nodeId"]
                .as_u64()
                .is_some_and(|node_id| node_id != 0)
            && !operation
                .target_selector
                .as_ref()
                .is_some_and(|selector| target["selector"] != selector.as_str())
            && catalog_frame_is_complete(pre_frame)
            && catalog_frame_is_complete(post_frame)
            && state.get("before").is_some()
            && state.get("after").is_some(),
    )
}

fn operation_evidence_identity_matches_request(
    evidence: &serde_json::Value,
    operation: &CatalogOperation,
    request: &FeatureRequest,
) -> Option<bool> {
    Some(
        evidence["schema"] == "krr-html84-operation-evidence-v1"
            && evidence["kind"] == operation.operation_kind
            && evidence["sourceSha256Before"] == request.source_sha256
            && evidence["sourceSha256After"] == request.source_sha256,
    )
}

fn catalog_unknown_reason(
    evidence: &serde_json::Value,
    font_audit: &serde_json::Value,
    fonts: &serde_json::Value,
    resources: &crate::renderer::backends::html_subresources::resource_audit::Inventory,
) -> String {
    let mut reason = operation_unknown_reason(evidence);
    if !resources.complete {
        reason.push_str("; resource inventory incomplete");
        if !resources.reason.is_empty() {
            reason.push_str(": ");
            reason.push_str(&resources.reason);
        }
    }
    append_font_audit_summary(&mut reason, font_audit);
    if !catalog_font_audit_complete(font_audit, fonts) {
        reason
            .push_str("; actual font audit is incomplete or has unverified physical face records");
    }
    reason
}

fn catalog_font_audit_complete(
    audit: &serde_json::Value,
    physical_faces: &serde_json::Value,
) -> bool {
    let Some(faces) = audit.get("faces").and_then(serde_json::Value::as_array) else {
        return false;
    };
    let Some(errors) = audit.get("errors").and_then(serde_json::Value::as_array) else {
        return false;
    };
    let Some(dropped) = audit.get("dropped").and_then(serde_json::Value::as_u64) else {
        return false;
    };
    !faces.is_empty()
        && errors.is_empty()
        && dropped == 0
        && faces
            .iter()
            .all(|face| catalog_font_record(face, Vec::new()).is_some())
        && final_glyph_audit_complete(audit)
        && physical_faces
            .as_array()
            .is_some_and(|records| !records.is_empty())
}

fn final_glyph_audit_complete(audit: &serde_json::Value) -> bool {
    audit
        .get("finalGlyphErrors")
        .and_then(serde_json::Value::as_array)
        .is_some_and(Vec::is_empty)
        && audit
            .get("finalGlyphDropped")
            .and_then(serde_json::Value::as_u64)
            == Some(0)
}

fn operation_unknown_reason(evidence: &serde_json::Value) -> String {
    if evidence["status"] == "unknown" {
        return evidence["reason"]
            .as_str()
            .unwrap_or("operation evidence is unsupported")
            .to_owned();
    }
    "operation or source-provenance evidence is incomplete".to_owned()
}

fn append_font_audit_summary(reason: &mut String, font_audit: &serde_json::Value) {
    let errors = font_audit_errors(font_audit);
    let dropped = audit_drop_count(font_audit);
    reason.push_str(&format!(
        "; actual font audit dropped={dropped}, errors=[{errors}]"
    ));
}

fn font_audit_errors(font_audit: &serde_json::Value) -> String {
    match font_audit
        .get("errors")
        .and_then(serde_json::Value::as_array)
    {
        Some(errors) => errors
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()
            .join(" | "),
        None => String::new(),
    }
}

struct CatalogReportEvidence {
    fonts: serde_json::Value,
    resource_inventory: crate::renderer::backends::html_subresources::resource_audit::Inventory,
    font_audit_complete: bool,
    reason: String,
    observed: Option<serde_json::Value>,
    diagnostic: Option<serde_json::Value>,
    target_visible: serde_json::Value,
}

fn build_catalog_report(
    request: &FeatureRequest,
    html: &str,
    operation: &CatalogOperation,
    capture: CatalogCapture,
    details: CatalogReportEvidence,
) -> serde_json::Value {
    let capture_complete = catalog_capture_is_complete(&capture, request, &details.target_visible);
    let status_observed = catalog_evidence_is_observed(
        details.observed.is_some(),
        details.resource_inventory.complete,
        details.font_audit_complete,
        capture_complete,
    );
    let status = if status_observed {
        "observed"
    } else {
        "unknown"
    };
    let mut report = catalog_report_envelope(request, html, operation, &capture, &details, status);
    apply_catalog_report_status(&mut report, &details, status_observed);
    report
}

fn apply_catalog_report_status(
    report: &mut serde_json::Value,
    details: &CatalogReportEvidence,
    status_observed: bool,
) {
    if status_observed {
        if let Some(mut observed) = details.observed.clone() {
            if let Some(object) = observed.as_object_mut() {
                object.insert("status".to_owned(), json!("observed"));
            }
            report["operationEvidence"] = observed;
        }
    } else {
        report["unknownReason"] = json!(&details.reason);
        report["operationEvidence"]["reason"] = json!(&details.reason);
        if let Some(diagnostic) = &details.diagnostic {
            report["diagnosticOperationEvidence"] = diagnostic.clone();
        }
    }
    if !status_observed && let Some(observed) = details.observed.as_ref() {
        report["observedOperationEvidence"] = observed.clone();
    }
}

fn catalog_report_envelope(
    request: &FeatureRequest,
    html: &str,
    operation: &CatalogOperation,
    capture: &CatalogCapture,
    details: &CatalogReportEvidence,
    status: &str,
) -> serde_json::Value {
    let operation_evidence = json!({"schema":"krr-html84-operation-evidence-v1","kind":operation.operation_kind,"status":status});
    json!({
        "schema":"krr-html84-raw-condition-v1", "conditionId":operation.condition_id,
        "status":status,
        "caseId":operation.case_id,"boundaryState":operation.boundary_state,
        "operationKind":operation.operation_kind,"htmlSha256":html_hash(html),
        "executionNonce":request.nonce,"binarySha256":&capture.binary_sha256,
        "state":&capture.state,"capturePath":&capture.capture,"captureSha256":&capture.capture_sha256,
        "viewport":{"width":request.width,"height":request.height,"scale":operation.viewport.scale},
        "geometry":&capture.geometry,"operationEvidence":operation_evidence,"frame":&capture.frame,
        "targetVisible":&details.target_visible,"resourceProvenance":{"inventoryComplete":details.resource_inventory.complete,"requests":&details.resource_inventory.requests},
        "fontProvenance":&details.fonts
    })
}

fn catalog_capture_is_complete(
    capture: &CatalogCapture,
    request: &FeatureRequest,
    target_visible: &serde_json::Value,
) -> bool {
    capture.capture.is_file()
        && fs::read(&capture.capture)
            .ok()
            .is_some_and(|bytes| digest(&bytes) == capture.capture_sha256)
        && is_sha256(&capture.capture_sha256)
        && capture.binary_sha256 == request.binary_sha
        && is_sha256(&capture.binary_sha256)
        && target_visible.as_bool() == Some(true)
        && catalog_frame_is_complete(&capture.frame)
        && capture.geometry.as_array().is_some_and(|items| {
            !items.is_empty() && items.iter().all(catalog_geometry_is_complete)
        })
}

fn catalog_frame_is_complete(frame: &serde_json::Value) -> bool {
    frame
        .get("generation")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|value| value > 0)
        && frame
            .get("contentHeight")
            .and_then(serde_json::Value::as_f64)
            .is_some_and(f64::is_finite)
        && frame
            .get("scrollY")
            .and_then(serde_json::Value::as_f64)
            .is_some_and(|value| value.is_finite() && value >= 0.0)
}

fn catalog_geometry_is_complete(geometry: &serde_json::Value) -> bool {
    geometry
        .get("nodeId")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|value| value > 0)
        && ["x", "y", "width", "height"].iter().all(|key| {
            geometry
                .get(*key)
                .and_then(serde_json::Value::as_f64)
                .is_some_and(f64::is_finite)
        })
        && geometry
            .get("width")
            .and_then(serde_json::Value::as_f64)
            .is_some_and(|value| value > 0.0)
        && geometry
            .get("height")
            .and_then(serde_json::Value::as_f64)
            .is_some_and(|value| value > 0.0)
}

fn catalog_evidence_is_observed(
    operation_observed: bool,
    resources_complete: bool,
    fonts_complete: bool,
    capture_complete: bool,
) -> bool {
    operation_observed && resources_complete && fonts_complete && capture_complete
}

fn write_catalog_report(request: &FeatureRequest, report: serde_json::Value) -> TestResult {
    validate_source_unchanged(request)?;
    fs::write(
        &request.output,
        serde_json::to_vec_pretty(&report).map_err(to_string)?,
    )
    .map_err(to_string)
}

fn apply_case_operation(session: &mut HtmlInteractiveSession, case_id: &str) -> TestResult {
    if case_id == "action-pointer" {
        return click_element(session, "action");
    }
    if case_id == "events" {
        click_element(session, "field")?;
        return apply_events_operation(session);
    }
    let Some(operation) = case_id.strip_prefix("input-state:") else {
        return Ok(());
    };
    apply_input_state_operation(session, operation)
}

fn apply_input_state_operation(
    session: &mut HtmlInteractiveSession,
    operation: &str,
) -> TestResult {
    match operation {
        "empty" | "disabled" => Ok(()),
        "filled" => {
            click_element(session, "name")?;
            session
                .dispatch_input(
                    crate::renderer::backends::html_browser::HtmlBrowserInput::Text {
                        text: "架空の名称".to_owned(),
                    },
                )
                .map_err(to_string)?;
            click_element(session, "save")
        }
        "selected" => click_element(session, "tab-advanced"),
        "expanded" => click_element(session, "accordion"),
        "focus-visible" => session
            .dispatch_input(
                crate::renderer::backends::html_browser::HtmlBrowserInput::KeyDown {
                    key: "Tab".to_owned(),
                },
            )
            .map_err(to_string),
        _ => Err(format!("unknown input-state operation: {operation}")),
    }
}

fn apply_events_operation(session: &mut HtmlInteractiveSession) -> TestResult {
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::Text {
                text: "入力".to_owned(),
            },
        )
        .map_err(to_string)?;
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::KeyDown {
                key: "Shift".to_owned(),
            },
        )
        .map_err(to_string)?;
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::KeyUp {
                key: "Shift".to_owned(),
            },
        )
        .map_err(to_string)?;
    let target = submit_target(session)?;
    let x = target.x + 1.0;
    let y = target.y + 1.0 - session.scroll_geometry.scroll_y;
    dispatch_pointer_pair(session, x, y)
}

fn submit_target(
    session: &mut HtmlInteractiveSession,
) -> TestResult<super::super::types::HitTarget> {
    let button = session
        .runtime
        .query_feature_node("button")
        .ok_or("missing submit button node")?;
    session
        .hit_targets
        .iter()
        .find(|target| target.node_id == button.0)
        .cloned()
        .ok_or_else(|| "missing submit button target".to_string())
}

fn dispatch_pointer_pair(session: &mut HtmlInteractiveSession, x: f32, y: f32) -> TestResult {
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::PointerDown {
                x,
                y,
                button: 0,
            },
        )
        .map_err(to_string)?;
    session
        .dispatch_input(
            crate::renderer::backends::html_browser::HtmlBrowserInput::PointerUp {
                x,
                y,
                button: 0,
            },
        )
        .map_err(to_string)
}

#[cfg(test)]
mod tests {
    use super::{CatalogOperation, CatalogViewport, FeatureRequest, feature_report};
    use serde_json::json;
    use std::path::PathBuf;

    const ACTION_POINTER_VIEWPORTS: [(u32, u32); 3] = [(1280, 900), (960, 720), (620, 900)];
    const ACTION_POINTER_FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/html/interaction-fidelity.html"
    ));
    const ASYNC_FIXTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/html/aspects/script-async.html"
    ));

    fn async_operation() -> CatalogOperation {
        CatalogOperation {
            schema: "krr-html84-operation-v1".to_owned(),
            condition_id: "async/expected-state/1280x900@1".to_owned(),
            case_id: "async".to_owned(),
            boundary_state: "expected-state".to_owned(),
            operation_kind: "script-await-timer-microtask".to_owned(),
            target_selector: None,
            viewport: CatalogViewport {
                width: 1280,
                height: 900,
                scale: 1.0,
            },
        }
    }

    fn async_request() -> Result<FeatureRequest, String> {
        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/html/aspects/script-async.html")
            .canonicalize()
            .map_err(|error| error.to_string())?;
        Ok(FeatureRequest {
            input,
            source_sha256: super::digest(ASYNC_FIXTURE),
            expression: "document.querySelector('#state').textContent".to_owned(),
            case_id: "async".to_owned(),
            nonce: "test-nonce".to_owned(),
            binary_sha: "test-binary".to_owned(),
            output: PathBuf::from("test-output"),
            width: 1280,
            height: 900,
            operation: None,
        })
    }

    #[test]
    fn async_timer_state_requires_execution_or_exact_startup_settlement() {
        let initial = serde_json::json!("initial");
        let expected = serde_json::json!(super::ASYNC_FIXTURE_EXPECTED_STATE);
        assert!(super::async_timer_state_matches(
            true, &initial, &expected, &expected
        ));
        assert!(super::async_timer_state_matches(
            false, &expected, &expected, &expected
        ));
        assert!(!super::async_timer_state_matches(
            false, &initial, &initial, &expected
        ));
        assert!(!super::async_timer_state_matches(
            true, &initial, &initial, &expected
        ));
        assert!(!super::async_timer_state_matches(
            false, &expected, &initial, &expected
        ));
    }

    #[test]
    fn startup_settlement_requires_the_pinned_async_source_and_operation() -> Result<(), String> {
        assert_eq!(super::digest(ASYNC_FIXTURE), super::ASYNC_FIXTURE_SHA256);
        let operation = async_operation();
        let request = async_request()?;
        assert!(super::is_pinned_async_timer_operation(
            &operation, &request
        )?);
        assert_wrong_async_source(&operation)?;
        assert_wrong_async_request(&operation)?;
        assert_wrong_async_operation(&request)?;
        Ok(())
    }

    fn assert_wrong_async_source(operation: &CatalogOperation) -> Result<(), String> {
        let mut wrong_source = async_request()?;
        wrong_source.source_sha256 = "different-source".to_owned();
        assert!(!super::is_pinned_async_timer_operation(
            operation,
            &wrong_source
        )?);
        Ok(())
    }

    fn assert_wrong_async_request(operation: &CatalogOperation) -> Result<(), String> {
        let mut wrong_request = async_request()?;
        wrong_request.case_id = "resources".to_owned();
        assert!(!super::is_pinned_async_timer_operation(
            operation,
            &wrong_request
        )?);

        let mut wrong_fixture = async_request()?;
        wrong_fixture.input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/html/aspects/script-resources.html")
            .canonicalize()
            .map_err(|error| error.to_string())?;
        assert!(!super::is_pinned_async_timer_operation(
            operation,
            &wrong_fixture
        )?);
        Ok(())
    }

    fn assert_wrong_async_operation(request: &FeatureRequest) -> Result<(), String> {
        let mut wrong_operation = async_operation();
        wrong_operation.operation_kind = "script-load-observe".to_owned();
        assert!(!super::is_pinned_async_timer_operation(
            &wrong_operation,
            request
        )?);
        Ok(())
    }

    fn all_toc_operation(target_selector: &str) -> CatalogOperation {
        CatalogOperation {
            schema: "krr-html84-operation-v1".to_owned(),
            condition_id: "navigation/all-toc-links/1280x900@1".to_owned(),
            case_id: "navigation".to_owned(),
            boundary_state: "all-toc-links".to_owned(),
            operation_kind: "navigation-all-pointer-clicks".to_owned(),
            target_selector: Some(target_selector.to_owned()),
            viewport: CatalogViewport {
                width: 1280,
                height: 900,
                scale: 1.0,
            },
        }
    }

    fn decoration_operation(state: &str, target_selector: &str) -> CatalogOperation {
        CatalogOperation {
            schema: "krr-html84-operation-v1".to_owned(),
            condition_id: format!("decoration/{state}/1280x900@1"),
            case_id: "decoration".to_owned(),
            boundary_state: state.to_owned(),
            operation_kind: "load-and-observe-target".to_owned(),
            target_selector: Some(target_selector.to_owned()),
            viewport: CatalogViewport {
                width: 1280,
                height: 900,
                scale: 1.0,
            },
        }
    }

    #[test]
    fn resource_settlement_requires_exact_fixture_state_before_and_after() {
        let expected = serde_json::json!({"state":"loaded","fonts":"ready"});
        assert!(super::resource_state_matches_fixture(
            &expected, &expected, &expected
        ));
        assert!(!super::resource_state_matches_fixture(
            &serde_json::json!({"state":"waiting","fonts":"ready"}),
            &expected,
            &expected,
        ));
        assert!(!super::resource_state_matches_fixture(
            &expected,
            &serde_json::json!({"state":"loaded","fonts":"loading"}),
            &expected,
        ));
    }

    #[test]
    fn decoration_selectors_match_each_finite_catalog_state() {
        for (state, selector) in [
            ("stacked", ".alert"),
            ("clipped", ".overlay"),
            ("translucent", ".overlay"),
        ] {
            assert!(
                super::validate_operation_inventory(&decoration_operation(state, selector)).is_ok()
            );
            assert!(
                super::validate_operation_inventory(&decoration_operation(
                    state,
                    ".alert,.overlay"
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn all_toc_selector_matches_the_catalog_and_rejects_other_spelling() {
        let canonical = all_toc_operation(".sidebar .toc-item[href^='#']");
        assert!(super::validate_operation_inventory(&canonical).is_ok());

        let other = all_toc_operation(".sidebar .toc-item[href^=\"#\"]");
        assert!(super::validate_operation_inventory(&other).is_err());
    }

    #[test]
    fn tab_operation_is_observed_only_when_focus_and_active_element_match() -> super::TestResult {
        let (mut session, target_id) = tab_session("<input id=name><button id=next>Next</button>")?;
        let mut trace = super::OperationTrace::default();

        super::apply_tab_observation(&mut session, target_id, &mut trace)?;

        let step = trace.steps.first().ok_or("Tab observation step missing")?;
        assert_eq!(step["focusedNodeAfter"], target_id);
        assert_eq!(step["activeElementAfter"], "name");
        assert_eq!(step["targetFocused"], true);
        assert_eq!(step["activeElementMatchesTarget"], true);
        assert_eq!(step["focusVisible"], true);
        assert!(!trace.extra.contains_key("status"));

        let html = "<input id=name><script>document.addEventListener('keydown', event => event.preventDefault());</script>";
        let (mut session, target_id) = tab_session(html)?;
        let mut trace = super::OperationTrace::default();
        super::apply_tab_observation(&mut session, target_id, &mut trace)?;
        assert_eq!(
            trace.extra.get("status"),
            Some(&serde_json::json!("unknown"))
        );
        Ok(())
    }

    fn tab_session(html: &str) -> super::TestResult<(super::HtmlInteractiveSession, u64)> {
        let mut session = super::super::support::start_with_viewport(html, 320, 240)?;
        let target_id = session
            .runtime
            .node_for_element_id("name")
            .map(|node| node.0)
            .ok_or("#name target missing")?;
        Ok((session, target_id))
    }

    fn assert_action_state(
        session: &mut super::HtmlInteractiveSession,
        expanded: &str,
        result: &str,
    ) -> super::TestResult {
        let actual = session.runtime.observe_feature_state(
            "({expanded:document.querySelector('#action').getAttribute('aria-expanded'),result:document.querySelector('#result').textContent})",
        ).map_err(super::to_string)?;
        assert_eq!(
            actual,
            serde_json::json!({"expanded": expanded, "result": result})
        );
        Ok(())
    }

    #[test]
    fn action_pointer_case_clicks_authored_fixture_at_all_performance_viewports()
    -> super::TestResult {
        for (width, height) in ACTION_POINTER_VIEWPORTS {
            let mut session =
                super::super::support::start_with_viewport(ACTION_POINTER_FIXTURE, width, height)?;
            assert_action_state(&mut session, "false", "")?;
            super::apply_case_operation(&mut session, "action-pointer")?;
            assert_action_state(&mut session, "true", "opened")?;
            super::apply_case_operation(&mut session, "action-pointer")?;
            assert_action_state(&mut session, "false", "closed")?;
        }
        Ok(())
    }

    #[test]
    fn feature_report_preserves_state_provenance_and_empty_geometry() {
        let request = FeatureRequest {
            input: PathBuf::from("fixture.html"),
            source_sha256: String::new(),
            expression: "true".to_owned(),
            case_id: "input-state:empty".to_owned(),
            nonce: "nonce".to_owned(),
            binary_sha: "binary".to_owned(),
            output: PathBuf::from("report.json"),
            width: 320,
            height: 240,
            operation: None,
        };
        let report = feature_report(
            &request,
            "<html></html>",
            serde_json::json!({"state": "ok"}),
            PathBuf::from("capture.png"),
            "capture-sha".to_owned(),
            &[],
        );
        assert_eq!(report["caseId"], "input-state:empty");
        assert_eq!(report["viewport"]["width"], 320);
        assert_eq!(report["captureSha256"], "capture-sha");
        assert!(
            report["geometry"]
                .as_array()
                .is_some_and(|items| items.is_empty())
        );
    }

    #[test]
    fn initial_frame_font_audit_is_attached_without_changing_existing_fields() {
        let mut report = serde_json::json!({
            "caseId": "input-state:empty",
            "geometry": [],
        });
        super::attach_initial_frame_font_audit(
            &mut report,
            serde_json::json!({"audit": "initial"}),
        );
        assert_eq!(report["caseId"], "input-state:empty");
        assert_eq!(report["geometry"], serde_json::json!([]));
        assert_eq!(report["initialFrameFontAudit"]["audit"], "initial");
    }

    #[test]
    fn catalog_writer_requires_operation_resource_font_and_capture_evidence() -> super::TestResult {
        assert_catalog_evidence_matrix();
        assert_catalog_font_audit_contract()?;
        Ok(())
    }

    fn assert_catalog_evidence_matrix() {
        assert!(super::catalog_evidence_is_observed(true, true, true, true));
        assert!(!super::catalog_evidence_is_observed(
            false, true, true, true
        ));
        assert!(!super::catalog_evidence_is_observed(
            true, false, true, true
        ));
        assert!(!super::catalog_evidence_is_observed(
            true, true, false, true
        ));
        assert!(!super::catalog_evidence_is_observed(
            true, true, true, false
        ));
    }

    fn assert_catalog_font_audit_contract() -> super::TestResult {
        let complete_audit = complete_catalog_font_audit();
        assert_complete_catalog_font_audit(&complete_audit);
        assert_catalog_layout_runs_are_face_bound_and_preserved(&complete_audit)?;
        assert_invalid_catalog_font_audit(&complete_audit);
        assert_missing_drop_count_is_rejected(&complete_audit);
        Ok(())
    }

    fn assert_catalog_layout_runs_are_face_bound_and_preserved(
        audit: &serde_json::Value,
    ) -> super::TestResult {
        let mut audit = audit.clone();
        audit["finalGlyphTrees"][0]["faces"][0]["faceId"] = json!("face-0");
        audit["finalGlyphTrees"][0]["textSpans"] = json!([
            {"faces":[{"faceId":"face-0","glyphCount":2,
                         "effectiveVariations":[],"derivedAppliedDesignCoordinates":[],"normalizedCoords":[]}],
             "layoutVariations":[{"tag":"wght","value":700.0}]},
            {"faces":[{"faceId":"face-0","glyphCount":1,
                         "effectiveVariations":[],"derivedAppliedDesignCoordinates":[],"normalizedCoords":[]}],
             "layoutVariations":[{"tag":"wdth","value":90.0}]}
        ]);
        let records = super::catalog_font_provenance(&audit);
        let runs = records
            .get(0)
            .and_then(|record| record["layoutRuns"].as_array())
            .ok_or("layout runs")?;
        assert_eq!(runs.len(), 2);
        assert!(runs.iter().all(|run| run["faceId"] == "face-0"));
        assert_eq!(runs[0]["paintCall"], 1);
        assert_eq!(runs[1]["glyphCount"], 1);
        Ok(())
    }

    fn complete_catalog_font_audit() -> serde_json::Value {
        let source = json!({
            "kind":"File", "path":"/font.ttf",
            "bytes_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "index":0, "families":["Example"]
        });
        json!({
            "faces":[source.clone()], "errors":[], "dropped":0,
            "finalGlyphTrees":[{"paintCall":1,"treePath":"root","faces":[{
                "identity":"physical_verified", "glyphCount":3, "source":source
            }]}],
            "finalGlyphErrors":[], "finalGlyphDropped":0, "finalGlyphPaintCalls":1
        })
    }

    fn assert_complete_catalog_font_audit(audit: &serde_json::Value) {
        let physical = super::catalog_font_provenance(audit);
        assert!(super::catalog_font_audit_complete(audit, &physical));
    }

    fn assert_invalid_catalog_font_audit(audit: &serde_json::Value) {
        let mut incomplete = audit.clone();
        incomplete["faces"][0]["bytes_sha256"] = json!("bad");
        let physical = super::catalog_font_provenance(&incomplete);
        assert!(!super::catalog_font_audit_complete(&incomplete, &physical));
    }

    fn assert_missing_drop_count_is_rejected(audit: &serde_json::Value) {
        let mut missing = audit.clone();
        if let Some(object) = missing.as_object_mut() {
            object.remove("dropped");
        }
        let physical = super::catalog_font_provenance(&missing);
        assert!(!super::catalog_font_audit_complete(&missing, &physical));
    }

    #[test]
    fn catalog_report_is_observed_only_with_complete_passive_audits() -> super::TestResult {
        let (root, capture_path) = catalog_report_capture_file()?;
        let report = catalog_report_fixture(&capture_path, true);
        assert_eq!(report["status"], "observed");
        assert_eq!(report["operationEvidence"]["status"], "observed");
        assert_eq!(report["resourceProvenance"]["inventoryComplete"], true);
        assert_eq!(report["resourceProvenance"]["requests"][0]["loaded"], true);

        let incomplete = catalog_report_fixture(&capture_path, false);
        assert_eq!(incomplete["status"], "unknown");
        assert_eq!(incomplete["operationEvidence"]["status"], "unknown");
        assert_eq!(
            incomplete["observedOperationEvidence"],
            serde_json::json!({"kind":"observed"})
        );
        assert_eq!(incomplete["resourceProvenance"]["inventoryComplete"], false);
        std::fs::remove_dir_all(root).map_err(super::to_string)?;
        Ok(())
    }

    #[test]
    fn unknown_timer_diagnostic_keeps_matching_state_without_acceptance() -> super::TestResult {
        let (root, capture_path) = catalog_report_capture_file()?;
        let mut request = async_request()?;
        request.binary_sha = super::digest(b"binary").to_owned();
        let operation = async_operation();
        let evidence = unknown_async_evidence(&request);
        let diagnostic = super::diagnostic_operation_evidence(&evidence, &operation, &request)
            .ok_or("bound unknown timer diagnostic was not preserved")?;
        assert!(super::observed_operation_evidence(&evidence, &operation, &request).is_none());
        let mut details = catalog_fixture_details(true);
        details.observed = None;
        details.diagnostic = Some(diagnostic);
        let report = super::build_catalog_report(
            &request,
            "<p/>",
            &operation,
            catalog_fixture_capture(&capture_path),
            details,
        );
        assert_unknown_timer_report(&report);
        std::fs::remove_dir_all(root).map_err(super::to_string)?;
        Ok(())
    }

    fn assert_unknown_timer_report(report: &serde_json::Value) {
        assert_eq!(report["status"], "unknown");
        assert_eq!(report["operationEvidence"]["status"], "unknown");
        assert_eq!(report["diagnosticOperationEvidence"]["status"], "unknown");
        assert_eq!(
            report["diagnosticOperationEvidence"]["state"]["before"],
            "microtask>ready>timer"
        );
        assert_eq!(
            report["diagnosticOperationEvidence"]["steps"][0]["timerRan"],
            false
        );
    }

    fn unknown_async_evidence(request: &FeatureRequest) -> serde_json::Value {
        let state = "microtask>ready>timer";
        json!({
            "schema":"krr-html84-operation-evidence-v1", "kind":"script-await-timer-microtask",
            "status":"unknown", "unknownReason":"timer delivery remains unverified",
            "reason":"timer delivery remains unverified",
            "sourceSha256Before":request.source_sha256, "sourceSha256After":request.source_sha256,
            "preFrame":{"generation":1,"contentHeight":100,"scrollY":0},
            "postFrame":{"generation":2,"contentHeight":100,"scrollY":0},
            "target":{"selector":null,"nodeId":1,"visible":true},
            "state":{"before":state,"after":state}, "delivery":"session-load-settled",
            "steps":[{"kind":"host-time-advance","elapsedMs":0.0,"timerRan":false,
                "microtaskCheckpoint":"runtime-advance-timers","stateBefore":state,
                "stateAfter":state,"fixtureStateVerified":false}]
        })
    }

    fn catalog_report_fixture(
        capture_path: &std::path::Path,
        resources_complete: bool,
    ) -> serde_json::Value {
        let request = catalog_report_request();
        let operation = all_toc_operation(".sidebar .toc-item[href^='#']");
        let capture = catalog_fixture_capture(capture_path);
        let details = catalog_fixture_details(resources_complete);
        super::build_catalog_report(&request, "<p/>", &operation, capture, details)
    }

    fn catalog_report_capture_file() -> super::TestResult<(PathBuf, PathBuf)> {
        let root = std::env::temp_dir().join(format!(
            "krr-catalog-writer-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(super::to_string)?
                .as_nanos()
        ));
        std::fs::create_dir(&root).map_err(super::to_string)?;
        let capture = root.join("capture.png");
        std::fs::write(&capture, b"capture").map_err(super::to_string)?;
        Ok((root, capture))
    }

    fn catalog_report_request() -> FeatureRequest {
        FeatureRequest {
            input: std::path::PathBuf::from("fixture.html"),
            source_sha256: "html-sha".to_owned(),
            expression: "true".to_owned(),
            case_id: "case".to_owned(),
            nonce: "nonce".to_owned(),
            binary_sha: super::digest(b"binary").to_owned(),
            output: std::path::PathBuf::from("report.json"),
            width: 320,
            height: 240,
            operation: None,
        }
    }

    fn catalog_fixture_capture(capture_path: &std::path::Path) -> super::CatalogCapture {
        super::CatalogCapture {
            state: serde_json::json!({"state":"after"}),
            frame: serde_json::json!({"generation":2,"contentHeight":240,"scrollY":0}),
            capture: capture_path.to_owned(),
            capture_sha256: super::digest(b"capture"),
            geometry: serde_json::json!([{"nodeId":1,"x":0,"y":0,"width":10,"height":10}]),
            binary_sha256: super::digest(b"binary"),
        }
    }

    fn catalog_fixture_details(resources_complete: bool) -> super::CatalogReportEvidence {
        super::CatalogReportEvidence {
            fonts: serde_json::json!([{"family":"fixture","source":"file","filePath":"/fixture.ttf","faceIndex":0,"fileSha256":"font-sha","scope":"capture","selected":true,"fileBytesVerified":true}]),
            resource_inventory:
                crate::renderer::backends::html_subresources::resource_audit::Inventory {
                    complete: resources_complete,
                    requests: serde_json::json!([{"url":"file:///fixture.css","bodyPath":"/fixture.css","bytesSha256":"resource-sha","loaded":true}]),
                    reason: if resources_complete {
                        String::new()
                    } else {
                        "fixture read failed".to_owned()
                    },
                },
            font_audit_complete: true,
            reason: "fixture audit reason".to_owned(),
            observed: Some(serde_json::json!({"kind":"observed"})),
            diagnostic: None,
            target_visible: serde_json::json!(true),
        }
    }
}

#[cfg(test)]
#[path = "feature_diagnostic_final_font_frame_tests.rs"]
mod final_font_frame_tests;

#[cfg(test)]
mod child_process_tests {
    use super::html_hash;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::time::{SystemTime, UNIX_EPOCH};

    const FEATURE_TEST_NAME: &str = "renderer::backends::html_interactive::tests::feature_diagnostic::writes_fixture_feature_state_interactive";

    fn child_workspace() -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "krr-feature-diagnostic-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).map_err(|error| error.to_string())?;
        Ok(path)
    }

    fn current_binary_sha() -> Result<String, String> {
        let bytes = std::fs::read(std::env::current_exe().map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        Ok(Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    fn run_feature_child(
        input: &Path,
        output: &Path,
        expression: &str,
        binary_sha: &str,
    ) -> Result<Output, String> {
        Command::new(std::env::current_exe().map_err(|error| error.to_string())?)
            .args(["--exact", FEATURE_TEST_NAME, "--nocapture"])
            .env("KRR_HTML_FEATURE_INPUT", input)
            .env("KRR_HTML_FEATURE_OUTPUT", output)
            .env("KRR_HTML_FEATURE_EXPRESSION", expression)
            .env("KRR_HTML_FEATURE_CASE", "input-state:filled")
            .env("KRR_HTML_FEATURE_NONCE", "child-test-nonce")
            .env("KRR_HTML_FEATURE_BINARY_SHA256", binary_sha)
            .env("KRR_HTML_FEATURE_WIDTH", "320")
            .env("KRR_HTML_FEATURE_HEIGHT", "240")
            .output()
            .map_err(|error| error.to_string())
    }

    fn assert_feature_output(output: &Path, html: &str, binary_sha: &str) -> Result<(), String> {
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        let capture = PathBuf::from(
            report["capturePath"]
                .as_str()
                .ok_or("capture path missing")?,
        );
        let png = std::fs::read(capture).map_err(|error| error.to_string())?;
        let png_sha = Sha256::digest(&png);
        let decoded = image::load_from_memory(&png).map_err(|error| error.to_string())?;
        assert_eq!((decoded.width(), decoded.height()), (320, 240));
        assert_eq!(report["htmlSha256"], json!(html_hash(html)));
        assert_eq!(report["executionNonce"], "child-test-nonce");
        assert_eq!(report["binarySha256"], binary_sha);
        assert_eq!(report["caseId"], "input-state:filled");
        assert_eq!(report["state"]["value"], "架空の名称");
        let expected_sha = png_sha
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(report["captureSha256"], expected_sha);
        assert!(!png_sha.is_empty());
        Ok(())
    }

    #[test]
    fn child_process_writes_state_png_and_provenance_for_post_operation() -> Result<(), String> {
        let workspace = child_workspace()?;
        let input = workspace.join("fixture.html");
        let output = workspace.join("feature.json");
        let html = "<input id=name><button id=save>Save</button>";
        std::fs::write(&input, html).map_err(|error| error.to_string())?;
        let binary_sha = current_binary_sha()?;
        let child = run_feature_child(
            &input,
            &output,
            "({value:document.querySelector('#name').value})",
            &binary_sha,
        )?;
        if !child.status.success() {
            return Err(format!(
                "feature child failed: {}",
                String::from_utf8_lossy(&child.stderr)
            ));
        }
        assert_feature_output(&output, html, &binary_sha)?;
        std::fs::remove_dir_all(workspace).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn child_process_reports_invalid_input_and_state_without_output() -> Result<(), String> {
        let workspace = child_workspace()?;
        let missing = workspace.join("missing.html");
        let output = workspace.join("invalid.json");
        let binary_sha = current_binary_sha()?;
        let child = run_feature_child(&missing, &output, "true", &binary_sha)?;
        assert!(!child.status.success());
        assert!(!output.exists());

        let input = workspace.join("fixture.html");
        std::fs::write(&input, "<p>valid</p>").map_err(|error| error.to_string())?;
        let child = run_feature_child(&input, &output, "missingVariable.value", &binary_sha)?;
        assert!(!child.status.success());
        assert!(!output.exists());
        std::fs::remove_dir_all(workspace).map_err(|error| error.to_string())?;
        Ok(())
    }
}
