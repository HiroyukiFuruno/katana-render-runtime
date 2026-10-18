use super::super::HtmlInteractiveSession;
use super::support::{TestResult, to_string};
use crate::renderer::backends::html_browser::{
    HtmlBrowserInput, HtmlBrowserSource, HtmlBrowserViewport,
};
use crate::renderer::backends::html_document::HtmlDocumentNode;
use serde_json::json;
use std::{collections::HashSet, fs};

#[test]
fn writes_fixture_layout_diagnostic_when_requested() -> TestResult {
    let Some(output) = std::env::var_os("KRR_HTML_LAYOUT_DIAGNOSTIC_OUTPUT") else {
        return Ok(());
    };
    create_diagnostic_directory(&output)?;
    let source = diagnostic_source()?;
    let viewport = HtmlBrowserViewport::new(
        diagnostic_dimension("KRR_HTML_LAYOUT_DIAGNOSTIC_WIDTH", 1280)?,
        diagnostic_dimension("KRR_HTML_LAYOUT_DIAGNOSTIC_HEIGHT", 900)?,
        1.0,
    )
    .map_err(to_string)?;
    let mut session = HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;
    apply_diagnostic_scroll(&mut session)?;
    let layout = session.layout().map_err(to_string)?;
    fs::write(
        std::path::Path::new(&output).with_extension("svg"),
        layout.svg,
    )
    .map_err(to_string)?;
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    let report = diagnostic_report(&session, snapshot, viewport);
    write_diagnostic_files(&output, &report, session.latest_frame())
}

fn diagnostic_source() -> TestResult<HtmlBrowserSource> {
    let path = std::env::var_os("KRR_HTML_LAYOUT_DIAGNOSTIC_INPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/html/library-reservation-fidelity.html")
        })
        .canonicalize()
        .map_err(to_string)?;
    let html = fs::read_to_string(&path).map_err(to_string)?;
    let origin =
        url::Url::from_file_path(path).map_err(|()| "invalid diagnostic origin".to_string())?;
    HtmlBrowserSource::new(html, origin.as_str()).map_err(to_string)
}

fn diagnostic_dimension(name: &str, default: u32) -> TestResult<u32> {
    match std::env::var(name) {
        Ok(width) => width.parse().map_err(to_string),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(to_string(error)),
    }
}

fn apply_diagnostic_scroll(session: &mut HtmlInteractiveSession) -> TestResult {
    let raw = match std::env::var("KRR_HTML_LAYOUT_DIAGNOSTIC_SCROLL") {
        Ok(raw) => raw,
        Err(std::env::VarError::NotPresent) => return Ok(()),
        Err(error) => return Err(to_string(error)),
    };
    let scroll: f32 = raw.parse().map_err(to_string)?;
    if !scroll.is_finite() || scroll < 0.0 {
        return Err("diagnostic scroll must be finite and nonnegative".into());
    }
    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: scroll,
        })
        .map_err(to_string)?;
    Ok(())
}

fn diagnostic_report(
    session: &HtmlInteractiveSession,
    snapshot: String,
    viewport: HtmlBrowserViewport,
) -> serde_json::Value {
    json!({
        "viewport": { "width": viewport.width, "height": viewport.height, "scale": viewport.device_scale_factor },
        "snapshot": snapshot,
        "element_boxes": diagnostic_boxes(session),
        "projected_elements": projected_elements(session, viewport.width as f32),
        "latest_frame": diagnostic_frame(session),
    })
}

fn diagnostic_boxes(session: &HtmlInteractiveSession) -> Vec<serde_json::Value> {
    session
        .element_boxes
        .iter()
        .map(|element| {
            json!({
                "node_id": element.node_id,
                "x": element.x,
                "y": element.y,
                "width": element.width,
                "height": element.height,
                "participates_in_flow": element.participates_in_flow,
            })
        })
        .collect()
}

fn diagnostic_frame(session: &HtmlInteractiveSession) -> Option<serde_json::Value> {
    session.latest_frame().map(|frame| {
        json!({
            "generation": frame.generation,
            "scroll_y": frame.scroll_y,
            "content_height": frame.content_height,
        })
    })
}

fn projected_elements(
    session: &HtmlInteractiveSession,
    viewport_width: f32,
) -> Vec<serde_json::Value> {
    let no_focused_nodes = HashSet::new();
    let Ok(nodes) = session.runtime.interactive_nodes_at_width_with_focus(
        viewport_width,
        &session.hovered_nodes,
        &no_focused_nodes,
        &no_focused_nodes,
    ) else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for node in &nodes {
        collect_projected_elements(node, &session.element_boxes, &mut values);
    }
    values
}

fn collect_projected_elements(
    node: &HtmlDocumentNode,
    boxes: &[crate::renderer::backends::html_interactive::types::ElementBox],
    values: &mut Vec<serde_json::Value>,
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
    let Some(layout) = boxes.iter().find(|value| value.node_id == *node_id) else {
        return;
    };
    values.push(projected_element_json(node_id, tag, attributes, layout));
    for child in children {
        collect_projected_elements(child, boxes, values);
    }
}

fn projected_element_json(
    node_id: &u64,
    tag: &str,
    attributes: &[(String, String)],
    layout: &crate::renderer::backends::html_interactive::types::ElementBox,
) -> serde_json::Value {
    let class = match attributes.iter().find(|(name, _)| name == "class") {
        Some((_, value)) => value.clone(),
        None => String::new(),
    };
    json!({
        "node_id": node_id,
        "tag": tag,
        "class": class,
        "x": layout.x,
        "y": layout.y,
        "width": layout.width,
        "height": layout.height,
    })
}

fn write_diagnostic_files(
    output: &std::ffi::OsStr,
    report: &serde_json::Value,
    frame: Option<&crate::renderer::backends::html_browser::HtmlBrowserFrame>,
) -> TestResult {
    fs::write(
        output,
        serde_json::to_vec_pretty(&report).map_err(to_string)?,
    )
    .map_err(to_string)?;
    if let Some(frame) = frame {
        let png = format!("{}.png", output.to_string_lossy());
        image::save_buffer_with_format(
            png,
            &frame.pixels,
            frame.viewport.width,
            frame.viewport.height,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(to_string)?;
    }
    Ok(())
}

fn create_diagnostic_directory(output: &std::ffi::OsStr) -> TestResult {
    if let Some(parent) = std::path::Path::new(output).parent() {
        fs::create_dir_all(parent).map_err(to_string)?;
    }
    Ok(())
}
