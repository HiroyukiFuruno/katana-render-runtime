use super::super::HtmlInteractiveSession;
use super::support::{TestResult, to_string};
use crate::renderer::backends::html_browser::{
    HtmlBrowserFrame, HtmlBrowserInput, HtmlBrowserSource, HtmlBrowserViewport,
};
use crate::renderer::backends::html_css::HtmlAttributes;
use crate::renderer::backends::html_css_selector::{CssAncestor, CssSelector};
use crate::renderer::backends::html_document::HtmlDocumentNode;
use crate::renderer::backends::html_interactive::types::LayoutResult;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[test]
fn writes_fixture_toc_diagnostic_when_requested() -> TestResult {
    let Some(output) = std::env::var_os("KRR_HTML_TOC_DIAGNOSTIC_OUTPUT") else {
        return Ok(());
    };
    let output = PathBuf::from(output);
    fs::create_dir_all(&output).map_err(to_string)?;
    let (source, width, height, scale) = diagnostic_source()?;
    let report = probe_viewport(&source, &output, width, height, scale)?;
    write_report(&output, &source, report)
}

fn diagnostic_source() -> TestResult<(HtmlBrowserSource, u32, u32, f32)> {
    let input = std::env::var_os("KRR_HTML_TOC_DIAGNOSTIC_INPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/html/anonymized-requirements.html")
        });
    let input = input.canonicalize().map_err(to_string)?;
    let html = fs::read_to_string(&input).map_err(to_string)?;
    let origin =
        url::Url::from_file_path(&input).map_err(|()| "invalid fixture origin".to_string())?;
    let source = HtmlBrowserSource::new(html, origin.as_str()).map_err(to_string)?;
    let width = diagnostic_dimension("KRR_HTML_TOC_DIAGNOSTIC_WIDTH", 1280)?;
    let height = diagnostic_dimension("KRR_HTML_TOC_DIAGNOSTIC_HEIGHT", 900)?;
    let scale = std::env::var("KRR_HTML_TOC_DIAGNOSTIC_SCALE")
        .unwrap_or_else(|_| "1".to_owned())
        .parse::<f32>()
        .map_err(to_string)?;
    Ok((source, width, height, scale))
}

fn write_report(output: &Path, source: &HtmlBrowserSource, report: Value) -> TestResult {
    let source_sha = Sha256::digest(source.raw_html.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let nonce = std::env::var("KRR_HTML_TOC_DIAGNOSTIC_NONCE").map_err(to_string)?;
    let report = json!({ "htmlSha256": source_sha, "executionNonce": nonce, "viewport": report["viewport"], "entries": report["entries"] });
    fs::write(
        output.join("toc-report.json"),
        serde_json::to_vec_pretty(&report).map_err(to_string)?,
    )
    .map_err(to_string)
}

fn probe_viewport(
    source: &HtmlBrowserSource,
    output: &Path,
    width: u32,
    height: u32,
    scale: f32,
) -> TestResult<Value> {
    let viewport = HtmlBrowserViewport::new(width, height, scale).map_err(to_string)?;
    let mut manifest_session =
        HtmlInteractiveSession::start(source.clone(), viewport).map_err(to_string)?;
    let entries = toc_entries(&mut manifest_session, width as f32)?;
    let captures = capture_entries(source, output, viewport, width, height, &entries)?;
    Ok(viewport_report(width, height, scale, captures))
}

fn capture_entries(
    source: &HtmlBrowserSource,
    output: &Path,
    viewport: HtmlBrowserViewport,
    width: u32,
    height: u32,
    entries: &[TocEntry],
) -> TestResult<Vec<Value>> {
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            capture_entry(
                source,
                CaptureContext {
                    output,
                    viewport,
                    width,
                    height,
                    index,
                    entry,
                },
            )
        })
        .collect()
}

fn viewport_report(width: u32, height: u32, scale: f32, entries: Vec<Value>) -> Value {
    json!({ "viewport": { "width": width, "height": height, "scale": scale }, "entries": entries })
}

fn capture_entry(source: &HtmlBrowserSource, context: CaptureContext<'_>) -> TestResult<Value> {
    let mut session =
        HtmlInteractiveSession::start(source.clone(), context.viewport).map_err(to_string)?;
    let (pointer_x, pointer_y, pointer_trace) = click_entry(&mut session, context.entry)?;
    assert_fragment(&session, &context.entry.hash)?;
    let prepared = prepare_capture(&mut session, context.entry, context.height)?;
    let mut capture = write_capture(&prepared.frame, &prepared.layout.svg, &context)?;
    capture["pointer"] = json!({ "x": pointer_x, "y": pointer_y });
    capture["pointerTrace"] = json!(pointer_trace);
    capture["targetTop"] = json!(prepared.target.1 - prepared.frame.scroll_y);
    capture["targetHeight"] = json!(prepared.target.3);
    Ok(capture)
}

struct PreparedCapture {
    frame: HtmlBrowserFrame,
    layout: LayoutResult,
    target: (f32, f32, f32, f32),
}

fn prepare_capture(
    session: &mut HtmlInteractiveSession,
    entry: &TocEntry,
    height: u32,
) -> TestResult<PreparedCapture> {
    let target = target_geometry(session, entry)?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "missing post-click frame".to_string())?
        .clone();
    if target.1 + target.3 <= frame.scroll_y || target.1 >= frame.scroll_y + height as f32 {
        return Err(format!(
            "#{} target is outside viewport after click",
            entry.hash
        ));
    }
    let frame_state = frame_state(&frame, session, target)?;
    let layout = session.layout().map_err(to_string)?;
    assert_same_state(session, &layout, &frame_state, entry)?;
    Ok(PreparedCapture {
        frame,
        layout,
        target,
    })
}

fn frame_state(
    frame: &HtmlBrowserFrame,
    session: &HtmlInteractiveSession,
    target: (f32, f32, f32, f32),
) -> TestResult<FrameState> {
    Ok(FrameState {
        scroll_y: frame.scroll_y,
        content_height: frame.content_height,
        generation: frame.generation,
        viewport: frame.viewport,
        fragment: current_fragment(session)?,
        target,
    })
}

fn target_geometry(
    session: &HtmlInteractiveSession,
    entry: &TocEntry,
) -> TestResult<(f32, f32, f32, f32)> {
    session
        .element_boxes
        .iter()
        .find(|box_| box_.node_id == entry.target_node_id)
        .map(|box_| (box_.x, box_.y, box_.width, box_.height))
        .ok_or_else(|| format!("missing target box for #{}", entry.hash))
}

fn layout_target_geometry(
    layout: &LayoutResult,
    entry: &TocEntry,
) -> TestResult<(f32, f32, f32, f32)> {
    layout
        .element_boxes
        .iter()
        .find(|box_| box_.node_id == entry.target_node_id)
        .map(|box_| (box_.x, box_.y, box_.width, box_.height))
        .ok_or_else(|| format!("missing SVG target box for #{}", entry.hash))
}

struct CaptureContext<'a> {
    output: &'a Path,
    viewport: HtmlBrowserViewport,
    width: u32,
    height: u32,
    index: usize,
    entry: &'a TocEntry,
}

struct FrameState {
    scroll_y: f32,
    content_height: f32,
    generation: u64,
    viewport: HtmlBrowserViewport,
    fragment: String,
    target: (f32, f32, f32, f32),
}

fn assert_same_state(
    session: &HtmlInteractiveSession,
    layout: &LayoutResult,
    state: &FrameState,
    entry: &TocEntry,
) -> TestResult {
    assert_frame_state(session, state)?;
    assert_session_layout_state(session, layout, state);
    assert_eq!(current_fragment(session)?, state.fragment);
    assert_eq!(layout_target_geometry(layout, entry)?, state.target);
    Ok(())
}

fn assert_frame_state(session: &HtmlInteractiveSession, state: &FrameState) -> TestResult {
    let refreshed_frame = session
        .latest_frame()
        .ok_or_else(|| "frame disappeared while writing SVG".to_string())?;
    assert!((refreshed_frame.scroll_y - state.scroll_y).abs() < 0.01);
    assert!((refreshed_frame.content_height - state.content_height).abs() < 0.01);
    assert_eq!(refreshed_frame.viewport, state.viewport);
    assert_eq!(refreshed_frame.generation, state.generation);
    Ok(())
}

fn assert_session_layout_state(
    session: &HtmlInteractiveSession,
    layout: &LayoutResult,
    state: &FrameState,
) {
    assert!((session.scroll_geometry.scroll_y - state.scroll_y).abs() < 0.01);
    assert!((session.scroll_geometry.content_height - state.content_height).abs() < 0.01);
    assert!((layout.content_height - state.content_height).abs() < 0.01);
    assert_eq!(session.viewport, state.viewport);
}

fn assert_fragment(session: &HtmlInteractiveSession, expected: &str) -> TestResult {
    let actual = current_fragment(session)?;
    (actual == expected)
        .then_some(())
        .ok_or_else(|| format!("expected #{expected}, got #{actual}"))
}

fn current_fragment(session: &HtmlInteractiveSession) -> TestResult<String> {
    session
        .source
        .origin
        .as_str()
        .split('#')
        .nth(1)
        .map(ToOwned::to_owned)
        .ok_or_else(|| "post-click source fragment is missing".to_string())
}

fn write_capture(
    frame: &HtmlBrowserFrame,
    svg: &str,
    context: &CaptureContext<'_>,
) -> TestResult<Value> {
    let capture_path = capture_path(context);
    write_png(frame, &capture_path, context)?;
    let capture_sha256 = Sha256::digest(fs::read(&capture_path).map_err(to_string)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let svg_path = capture_path.with_extension("svg");
    let svg_sha256 = write_svg(svg, &svg_path)?;
    Ok(capture_report(
        frame,
        context,
        capture_path,
        capture_sha256,
        svg_path,
        svg_sha256,
    ))
}

fn capture_path(context: &CaptureContext<'_>) -> PathBuf {
    context.output.join(format!(
        "toc-{}x{}-{:02}-{}.png",
        context.width, context.height, context.index, context.entry.hash
    ))
}

fn write_png(frame: &HtmlBrowserFrame, path: &Path, context: &CaptureContext<'_>) -> TestResult {
    image::save_buffer_with_format(
        path,
        &frame.pixels,
        context.width,
        context.height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(to_string)
}

fn write_svg(svg: &str, path: &Path) -> TestResult<String> {
    fs::write(path, svg).map_err(to_string)?;
    Ok(Sha256::digest(svg.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn capture_report(
    frame: &HtmlBrowserFrame,
    context: &CaptureContext<'_>,
    capture_path: PathBuf,
    capture_sha256: String,
    svg_path: PathBuf,
    svg_sha256: String,
) -> Value {
    json!({
        "destination": context.entry.hash,
        "hash": format!("#{}", context.entry.hash),
        "scrollY": frame.scroll_y,
        "capturePath": capture_path,
        "captureSha256": capture_sha256,
        "targetVisible": true,
        "svgPath": svg_path,
        "svgSha256": svg_sha256
    })
}

fn click_entry(
    session: &mut HtmlInteractiveSession,
    entry: &TocEntry,
) -> TestResult<(f32, f32, Vec<Value>)> {
    let click = session
        .hit_targets
        .iter()
        .find(|hit| hit.node_id == entry.node_id)
        .cloned()
        .ok_or_else(|| format!("missing hit target for #{}", entry.hash))?;
    let x = (click.x + click.width / 2.0).max(1.0);
    let y = (click.y + click.height / 2.0 - session.scroll_geometry.scroll_y).max(1.0);
    let mut trace = Vec::new();
    for (kind, input) in [
        ("move", HtmlBrowserInput::PointerMove { x, y }),
        ("down", HtmlBrowserInput::PointerDown { x, y, button: 0 }),
        ("up", HtmlBrowserInput::PointerUp { x, y, button: 0 }),
    ] {
        let pre = session.latest_frame().ok_or("missing pre-pointer frame")?;
        let pre_frame = json!({"generation":pre.generation,"scrollY":pre.scroll_y});
        session.dispatch_input(input).map_err(to_string)?;
        let post = session.latest_frame().ok_or("missing post-pointer frame")?;
        trace.push(json!({"kind":kind,"targetNodeId":entry.node_id,"x":x,"y":y,"preFrame":pre_frame,"postFrame":{"generation":post.generation,"scrollY":post.scroll_y}}));
    }
    Ok((x, y, trace))
}

pub(super) fn capture_catalog_all_toc_links(
    session: &mut HtmlInteractiveSession,
    output: &Path,
    width: u32,
    height: u32,
    scale: f32,
) -> TestResult<Vec<Value>> {
    fs::create_dir_all(output).map_err(to_string)?;
    let entries = toc_entries(session, width as f32)?;
    validate_catalog_toc_request(session, &entries, width, height, scale)?;
    let source = session.source.clone();
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            capture_catalog_toc_link(session, output, width, height, index, entry, &source)
        })
        .collect()
}

fn validate_catalog_toc_request(
    session: &HtmlInteractiveSession,
    entries: &[TocEntry],
    width: u32,
    height: u32,
    scale: f32,
) -> TestResult {
    let expected = ["a", "b", "c"];
    if entries.len() != expected.len()
        || entries
            .iter()
            .zip(expected)
            .any(|(entry, wanted)| entry.hash != wanted)
    {
        return Err("catalog all-TOC target inventory differs from #a, #b, #c".into());
    }
    let viewport = session.viewport;
    if viewport.width != width || viewport.height != height || viewport.device_scale_factor != scale
    {
        return Err("all-TOC capture viewport differs from the catalog condition".into());
    }
    Ok(())
}

struct CatalogTocClick {
    hash_before: String,
    pointer_trace: Vec<Value>,
    prepared: PreparedCapture,
}

fn prepare_catalog_toc_click(
    session: &mut HtmlInteractiveSession,
    entry: &TocEntry,
    height: u32,
) -> TestResult<CatalogTocClick> {
    let hash_before = current_location_hash(session);
    let (_, _, pointer_trace) = click_entry(session, entry)?;
    assert_fragment(session, &entry.hash)?;
    let prepared = prepare_capture(session, entry, height)?;
    Ok(CatalogTocClick {
        hash_before,
        pointer_trace,
        prepared,
    })
}

fn capture_catalog_toc_link(
    session: &mut HtmlInteractiveSession,
    output: &Path,
    width: u32,
    height: u32,
    index: usize,
    entry: &TocEntry,
    source: &HtmlBrowserSource,
) -> TestResult<Value> {
    let click = prepare_catalog_toc_click(session, entry, height)?;
    let context = CaptureContext {
        output,
        viewport: session.viewport,
        width,
        height,
        index,
        entry,
    };
    let mut record = write_capture(&click.prepared.frame, &click.prepared.layout.svg, &context)?;
    attach_catalog_toc_evidence(
        &mut record,
        &click.prepared.frame,
        &click.pointer_trace,
        &click.hash_before,
        entry,
        source,
    );
    Ok(record)
}

fn attach_catalog_toc_evidence(
    record: &mut Value,
    frame: &HtmlBrowserFrame,
    pointer_trace: &[Value],
    hash_before: &str,
    entry: &TocEntry,
    source: &HtmlBrowserSource,
) {
    record["destination"] = json!(format!("#{}", entry.hash));
    record["hashBefore"] = json!(hash_before);
    record["hashAfter"] = json!(format!("#{}", entry.hash));
    record["scrollY"] = json!(frame.scroll_y);
    record["frame"] = json!({"generation":frame.generation,"contentHeight":frame.content_height,"scrollY":frame.scroll_y});
    record["pointerTrace"] = json!(pointer_trace);
    record["sourceSha256"] = json!(
        Sha256::digest(source.raw_html.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
}

fn current_location_hash(session: &HtmlInteractiveSession) -> String {
    match session.source.origin.url().fragment() {
        Some(fragment) => format!("#{fragment}"),
        None => String::new(),
    }
}

fn diagnostic_dimension(name: &str, default: u32) -> TestResult<u32> {
    match std::env::var(name) {
        Ok(value) => value.parse().map_err(to_string),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(to_string(error)),
    }
}

struct TocEntry {
    node_id: u64,
    target_node_id: u64,
    hash: String,
}

fn toc_entries(session: &mut HtmlInteractiveSession, width: f32) -> TestResult<Vec<TocEntry>> {
    let selectors = toc_selectors()?;
    let no_focused_nodes = HashSet::new();
    let nodes = session
        .runtime
        .interactive_nodes_at_width_with_focus(
            width,
            &session.hovered_nodes,
            &no_focused_nodes,
            &no_focused_nodes,
        )
        .map_err(to_string)?;
    let mut entries = Vec::new();
    for node in &nodes {
        collect_entries(
            node,
            &mut session.runtime,
            &selectors,
            &mut Vec::new(),
            &mut entries,
        );
    }
    Ok(entries)
}

fn toc_selectors() -> TestResult<[CssSelector; 2]> {
    let legacy = CssSelector::parse(".sidebar .toc-item[href^='#']")
        .ok_or_else(|| "invalid legacy TOC selector".to_string())?;
    let navigation = CssSelector::parse("nav a[href^='#']")
        .ok_or_else(|| "invalid navigation TOC selector".to_string())?;
    Ok([legacy, navigation])
}

fn collect_entries(
    node: &HtmlDocumentNode,
    runtime: &mut crate::renderer::backends::html_runtime::StaticHtmlRuntimeSession,
    selectors: &[CssSelector],
    ancestors: &mut Vec<CssAncestor>,
    entries: &mut Vec<TocEntry>,
) {
    let HtmlDocumentNode::Element {
        node_id,
        tag,
        attributes,
        children,
    } = node
    else {
        return;
    };
    if tag == "a" {
        collect_toc_anchor(*node_id, attributes, selectors, ancestors, runtime, entries);
    }
    ancestors.push(CssAncestor::new_at_state_with_type(
        tag, attributes, 1, 1, 1, false,
    ));
    for child in children {
        collect_entries(child, runtime, selectors, ancestors, entries);
    }
    ancestors.pop();
}

fn collect_toc_anchor(
    node_id: u64,
    attributes: &HtmlAttributes,
    selectors: &[CssSelector],
    ancestors: &[CssAncestor],
    runtime: &mut crate::renderer::backends::html_runtime::StaticHtmlRuntimeSession,
    entries: &mut Vec<TocEntry>,
) {
    let Some(href) = attributes
        .iter()
        .find(|(name, _)| name == "href")
        .map(|(_, value)| value)
        .filter(|value| value.starts_with('#'))
    else {
        return;
    };
    if !is_toc_anchor(attributes, selectors, ancestors) {
        return;
    }
    let hash = href[1..].to_owned();
    if let Some(target_node_id) = runtime.node_for_element_id(&hash).map(|id| id.0) {
        entries.push(TocEntry {
            node_id,
            target_node_id,
            hash,
        });
    }
}

fn is_toc_anchor(
    attributes: &HtmlAttributes,
    selectors: &[CssSelector],
    ancestors: &[CssAncestor],
) -> bool {
    selectors
        .iter()
        .any(|selector| selector.matches_at_with_type("a", attributes, ancestors, 1, 1, 1))
}

#[test]
fn collects_navigation_and_legacy_toc_links_once_in_document_order() -> TestResult {
    let source = HtmlBrowserSource::new(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/html-harness/toc-selector-contract.html"
        )),
        "https://example.test/toc.html",
    )
    .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(320, 240, 1.0).map_err(to_string)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;
    let entries = toc_entries(&mut session, 320.0)?;
    let hashes = entries
        .iter()
        .map(|entry| entry.hash.as_str())
        .collect::<Vec<_>>();
    assert_eq!(hashes, ["legacy", "both", "nav"]);
    Ok(())
}
