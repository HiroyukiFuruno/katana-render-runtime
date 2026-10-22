use super::super::super::{
    diagram_type::MermaidDiagramType, js_runtime_scripts::MermaidRuntimeScripts,
};
use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};
use crate::markdown::runtime_assets::RuntimeAsset;
use serde_json::Value;

pub(super) struct MeasurementFixture {
    pub(super) calls: Vec<Value>,
    pub(super) boxes: Vec<Value>,
    pub(super) probes: Value,
}

pub(super) fn render_fixture(source: &str) -> Result<MeasurementFixture, String> {
    let mermaid = RuntimeAsset::mermaid();
    let path = mermaid.materialize_at(mermaid.materialized_path())?;
    let bundle = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let diagram = MermaidDiagramType::from_source(source);
    let mut scripts = MermaidRuntimeScripts::build(&bundle, &request(source, diagram), diagram);
    insert_trace(&mut scripts)?;
    scripts.push(DiagramRuntimeScript::owned(
        "native-width-result.js",
        result_script().into(),
    ));
    let output = DiagramV8Runtime::render(&scripts)?;
    decode_fixture(&output)
}

fn decode_fixture(output: &str) -> Result<MeasurementFixture, String> {
    let result: Value = serde_json::from_str(output).map_err(|error| error.to_string())?;
    let calls = result
        .get("calls")
        .and_then(Value::as_array)
        .cloned()
        .ok_or("actual native width calls are missing")?;
    let boxes = result
        .get("boxes")
        .and_then(Value::as_array)
        .cloned()
        .ok_or("actual getBBox observations are missing")?;
    let probes = result
        .get("probes")
        .cloned()
        .ok_or("probe metadata missing")?;
    Ok(MeasurementFixture {
        calls,
        boxes,
        probes,
    })
}

fn request(source: &str, diagram: MermaidDiagramType) -> String {
    serde_json::json!({
        "source": source, "svgId": "native-width-probe", "theme": "dark",
        "background": "#000", "fill": "#111", "text": "#fff",
        "stroke": "#fff", "arrow": "#fff", "diagramType": diagram.request_value(),
    })
    .to_string()
}

fn insert_trace(scripts: &mut Vec<DiagramRuntimeScript<'_>>) -> Result<(), String> {
    let index = scripts
        .iter()
        .position(|script| script.name == "render-mermaid.js")
        .ok_or("Mermaid render entrypoint missing")?;
    scripts.insert(
        index,
        DiagramRuntimeScript::borrowed(
            "native-width-trace.js",
            include_str!("js_runtime_native_width_trace.js"),
        ),
    );
    scripts.insert(
        index + 1,
        DiagramRuntimeScript::borrowed(
            "native-width-bbox-trace.js",
            include_str!("js_runtime_native_width_bbox_trace.js"),
        ),
    );
    Ok(())
}

fn result_script() -> &'static str {
    "if (globalThis.__krrActualBBoxTraceOverflow) throw new Error('native width getBBox trace overflow'); if (globalThis.__krrActualBBoxTraceFailure) throw new Error('native width getBBox snapshot failed: ' + globalThis.__krrActualBBoxTraceFailure); JSON.stringify({ calls: globalThis.__krrActualWidths, boxes: globalThis.__krrActualBBoxes, probes: nativeWidthProbeResult() })"
}
