use super::super::super::{
    diagram_type::MermaidDiagramType, js_runtime_scripts::MermaidRuntimeScripts,
};
use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};
use crate::markdown::runtime_resource_loader::{BundledResource, RuntimeResourceLoader};
use serde_json::Value;

pub(super) struct MeasurementFixture {
    pub(super) calls: Vec<Value>,
    pub(super) boxes: Vec<Value>,
    pub(super) c4_description_rows: Vec<Value>,
    pub(super) svg: String,
    pub(super) probes: Value,
}

pub(super) fn render_fixture(source: &str) -> Result<MeasurementFixture, String> {
    render_fixture_with_targets(source, &[])
}

fn render_fixture_with_targets(
    source: &str,
    targets: &[(&str, &str)],
) -> Result<MeasurementFixture, String> {
    let bundle = RuntimeResourceLoader::read_bundled(BundledResource::Mermaid)
        .map_err(|error| format!("Failed to read Mermaid.js bundle: {error}"))?;
    let diagram = MermaidDiagramType::from_source(source);
    let mut scripts = MermaidRuntimeScripts::build(bundle, &request(source, diagram), diagram);
    install_trace(&mut scripts, targets)?;
    scripts.push(DiagramRuntimeScript::owned(
        "native-width-result.js",
        result_script().into(),
    ));
    let output = DiagramV8Runtime::render(&scripts)?;
    decode_fixture(&output)
}

fn install_trace(
    scripts: &mut Vec<DiagramRuntimeScript<'_>>,
    targets: &[(&str, &str)],
) -> Result<(), String> {
    let index = scripts
        .iter()
        .position(|script| script.name == "render-mermaid.js")
        .ok_or("Mermaid render entrypoint missing")?;
    let targets = serde_json::json!(targets);
    let prelude = [
        DiagramRuntimeScript::borrowed(
            "native-width-trace.js",
            include_str!("js_runtime_native_width_trace.js"),
        ),
        DiagramRuntimeScript::borrowed(
            "native-width-bbox-trace.js",
            include_str!("js_runtime_native_width_bbox_trace.js"),
        ),
        DiagramRuntimeScript::owned(
            "native-width-targets.js",
            format!("globalThis.__krrTraceTargets = {targets};"),
        ),
    ];
    let entry_index = index + prelude.len();
    drop(scripts.splice(index..index, prelude));
    capture_rendered_svg(&mut scripts[entry_index]);
    Ok(())
}

fn capture_rendered_svg(entry: &mut DiagramRuntimeScript<'_>) {
    entry.code = format!(
        "Promise.resolve({}).then(svg => {{ globalThis.__krrRenderedSvg = svg; return svg; }})",
        entry.code.trim_end_matches(';')
    )
    .into();
}

pub(super) fn render_fixture_with_font_audit(
    source: &str,
) -> Result<(MeasurementFixture, Value), String> {
    render_fixture_targets_with_font_audit(source, &[])
}

pub(super) fn render_fixture_targets_with_font_audit(
    source: &str,
    targets: &[(&str, &str)],
) -> Result<(MeasurementFixture, Value), String> {
    #[cfg(target_os = "macos")]
    {
        let (result, audit) =
            crate::markdown::svg_rasterize::SvgRasterizeOps::with_actual_font_source_audit(|| {
                render_fixture_with_targets(source, targets)
            })
            .map_err(|error| error.to_string())?;
        Ok((result?, audit))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok((
            render_fixture_with_targets(source, targets)?,
            serde_json::json!({
                "available": false,
                "faces": [],
                "route": "non-macos-no-audit"
            }),
        ))
    }
}

fn decode_fixture(output: &str) -> Result<MeasurementFixture, String> {
    let result: Value = serde_json::from_str(output).map_err(|error| error.to_string())?;
    let probes = result
        .get("probes")
        .cloned()
        .ok_or("probe metadata missing")?;
    Ok(MeasurementFixture {
        calls: observation_array(&result, "calls", "actual native width calls are missing")?,
        boxes: observation_array(&result, "boxes", "actual getBBox observations are missing")?,
        c4_description_rows: observation_array(
            &result,
            "c4DescriptionRows",
            "C4 description observations are missing",
        )?,
        svg: result
            .get("svg")
            .and_then(Value::as_str)
            .ok_or("rendered SVG missing")?
            .to_owned(),
        probes,
    })
}

fn observation_array(result: &Value, key: &str, error: &str) -> Result<Vec<Value>, String> {
    result
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| error.to_owned())
}

fn request(source: &str, diagram: MermaidDiagramType) -> String {
    serde_json::json!({
        "source": source, "svgId": "native-width-probe", "theme": "dark",
        "background": "#000", "fill": "#111", "text": "#fff",
        "stroke": "#fff", "arrow": "#fff", "diagramType": diagram.request_value(),
    })
    .to_string()
}

fn result_script() -> &'static str {
    "if (globalThis.__krrActualBBoxTraceOverflow) throw new Error('native width getBBox trace overflow'); if (globalThis.__krrActualBBoxTraceFailure) throw new Error('native width getBBox snapshot failed: ' + globalThis.__krrActualBBoxTraceFailure); JSON.stringify({ calls: globalThis.__krrActualWidths, boxes: globalThis.__krrActualBBoxes, c4DescriptionRows: globalThis.__krrC4DescriptionRows, svg: globalThis.__krrRenderedSvg, probes: nativeWidthProbeResult() })"
}

#[cfg(test)]
#[path = "js_runtime_native_width_decode_tests.rs"]
mod decode_tests;
