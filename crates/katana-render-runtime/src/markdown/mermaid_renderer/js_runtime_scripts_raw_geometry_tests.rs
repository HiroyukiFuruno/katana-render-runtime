use super::{DiagramRuntimeScript, MermaidRuntimeScripts};
use crate::markdown::{diagram_js_runtime::DiagramV8Runtime, runtime_assets::RuntimeAsset};
#[path = "js_runtime_scripts_raw_geometry_c4_observer_test_support.rs"]
mod c4_geometry_observer;
#[path = "js_runtime_scripts_raw_geometry_class_git_diagnostic_tests.rs"]
mod class_git_diagnostic;

#[path = "js_runtime_scripts_raw_geometry_diagnostic_output.rs"]
mod diagnostic_output;
#[path = "js_runtime_scripts_raw_geometry_er_attribute_tests.rs"]
mod er_attribute_tests;
#[path = "js_runtime_scripts_raw_geometry_tree_tests.rs"]
mod geometry_tree;
#[path = "js_runtime_scripts_raw_geometry_sequence_diagnostic_tests.rs"]
mod sequence_diagnostic;
use geometry_tree::{first_geometry_difference, normalize_permitted_changes, raw_geometry_tree};

#[path = "js_runtime_scripts_raw_geometry_structure_tests.rs"]
mod geometry_structure;
#[path = "js_runtime_scripts_raw_geometry_layout_tests.rs"]
mod layout_tests;
#[path = "js_runtime_scripts_raw_geometry_vendor_layout_tests.rs"]
mod vendor_layout_tests;

const RAW_TEST_SVG_ID: &str = "vendor-raw-geometry-contract";
fn raw_geometry_request(source: &str) -> serde_json::Value {
    serde_json::json!({
        "source": source, "svgId": RAW_TEST_SVG_ID,
        "theme": "dark", "background": "#000", "fill": "#111",
        "text": "#fff", "stroke": "#fff", "arrow": "#fff",
    })
}

fn raw_geometry_vendor_config() -> serde_json::Value {
    serde_json::json!({
        "startOnLoad": false, "securityLevel": "loose", "htmlLabels": false, "theme": "dark",
        "flowchart": {"htmlLabels": false, "useMaxWidth": false},
        "sequence": {"useMaxWidth": false},
        "themeVariables": {
            "background":"#000", "mainBkg":"#111", "primaryColor":"#111",
            "primaryTextColor":"#fff", "primaryBorderColor":"#fff", "secondaryColor":"#111",
            "secondaryTextColor":"#fff", "secondaryBorderColor":"#fff", "tertiaryColor":"#111",
            "tertiaryTextColor":"#fff", "tertiaryBorderColor":"#fff", "nodeTextColor":"#fff",
            "lineColor":"#fff", "textColor":"#fff", "edgeLabelBackground":"#111",
            "actorBkg":"#111", "actorTextColor":"#fff", "actorBorder":"#fff",
            "signalColor":"#fff", "signalTextColor":"#fff", "labelTextColor":"#fff",
            "noteBkgColor":"#111", "noteTextColor":"#fff", "noteBorderColor":"#fff",
            "clusterBkg":"#000", "clusterBorder":"#fff", "titleColor":"#fff",
        },
    })
}

fn render_vendor_raw_and_processed(
    request: &serde_json::Value,
    config: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    render_vendor_raw_and_processed_mode(request, config, false, &[], &[])
}

pub(super) fn render_vendor_raw_and_processed_through_runtime(
    request: &serde_json::Value,
    config: &serde_json::Value,
    actor_labels: &[&str],
) -> Result<serde_json::Value, String> {
    render_vendor_raw_and_processed_mode(request, config, true, actor_labels, &[])
}

pub(super) fn render_vendor_raw_and_processed_with_metric_labels(
    request: &serde_json::Value,
    config: &serde_json::Value,
    metric_labels: &[&str],
) -> Result<serde_json::Value, String> {
    render_vendor_raw_and_processed_mode(request, config, true, &[], metric_labels)
}

fn render_vendor_raw_and_processed_mode(
    request: &serde_json::Value,
    config: &serde_json::Value,
    production_entrypoint: bool,
    actor_labels: &[&str],
    metric_labels: &[&str],
) -> Result<serde_json::Value, String> {
    let bytes = RuntimeAsset::mermaid()
        .bytes()
        .map_err(|error| error.to_string())?;
    let vendor = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let mut scripts = MermaidRuntimeScripts::build_with_zenuml(vendor, "", &request.to_string());
    let last = scripts
        .last_mut()
        .ok_or_else(|| "runtime scripts missing".to_string())?;
    *last = DiagramRuntimeScript::owned(
        "raw-vendor-geometry-test.js",
        raw_geometry_driver(
            request,
            config,
            production_entrypoint,
            actor_labels,
            metric_labels,
        ),
    );
    let envelope = DiagramV8Runtime::render(&scripts)?;
    serde_json::from_str(&envelope).map_err(|error| error.to_string())
}

/* WHY: rawはvendor返値の不変コピーとし、正規化結果から期待形状を作らない。 */
fn raw_geometry_driver(
    request: &serde_json::Value,
    config: &serde_json::Value,
    production_entrypoint: bool,
    actor_labels: &[&str],
    metric_labels: &[&str],
) -> String {
    let actor_labels = serde_json::json!(actor_labels);
    let metric_labels = serde_json::json!(metric_labels);
    let body = format!(
        "{RAW_GEOMETRY_DRIVER_PREFIX}{}{RAW_GEOMETRY_DRIVER_SUFFIX}",
        c4_geometry_observer::SOURCE
    );
    format!(
        "(async () => {{const request = {request}; const config = {config}; const productionEntryPoint = {production_entrypoint}; const actorLabels = {actor_labels}; const metricLabels = {metric_labels};\n{body}\n}})();"
    )
}

const RAW_GEOMETRY_DRIVER_PREFIX: &str = r#"const vendor = globalThis.mermaid ?? globalThis.window.mermaid;
if (!vendor || typeof vendor.render !== 'function') throw new Error('vendor absent');
let initializeInput, renderInput, svgId, renderCalls = 0;
let fontMeasureCalls = 0, svgFontMetricCalls = 0;
const svgFontMetricObservations = [];
let selectedMetricCalls = 0, selectedMetricDuplicateCalls = 0;
const svgFontMetricTraceLimit = 32;
const recordAllSvgFontMetrics = metricLabels.length === 0;
const svgTextBoxObservations = [];
const svgTextBoxTraceLimit = 16;
const originalInitialize = vendor.initialize.bind(vendor);
const originalRender = vendor.render.bind(vendor);
const originalMeasureText = globalThis.__katanaMeasureText;
const originalMeasureSvgTextMetrics = globalThis.__katanaMeasureSvgTextMetrics;
if (productionEntryPoint && (typeof originalMeasureText !== 'function' || typeof originalMeasureSvgTextMetrics !== 'function'))
  throw new Error('native font callbacks absent');
if (actorLabels.length > 0) {
  const documentBody = globalThis.document?.body;
  const elementPrototype = documentBody && Object.getPrototypeOf(documentBody);
  const originalGetBBox = elementPrototype?.getBBox;
  if (typeof originalGetBBox !== 'function') throw new Error('DOM getBBox absent');
  elementPrototype.getBBox = function(...args) {
    const parent = this.parentNode;
    const body = globalThis.document?.body;
    const text = String(this.textContent ?? '');
    const isActorMeasure = this.localName === 'text' && parent?.localName === 'svg'
      && parent.parentNode === body && actorLabels.includes(text);
    const box = originalGetBBox.apply(this, args);
    if (!isActorMeasure) return box;
    if (svgTextBoxObservations.length >= svgTextBoxTraceLimit)
      throw new Error('actor SVG text-box diagnostic trace limit exceeded');
    const attributeNames = ['class', 'x', 'y', 'text-anchor', 'font-family', 'font-size', 'font-weight', 'style'];
    const styleNames = ['font-family', 'font-size', 'font-weight', 'font-style', 'line-height'];
    const attributes = Object.fromEntries(attributeNames.map(name => [name, this.getAttribute(name)]));
    const styles = Object.fromEntries(styleNames.map(name => [name, this.style?.getPropertyValue(name) ?? '']));
    svgTextBoxObservations.push({
      ordinal: svgTextBoxObservations.length,
      node: {tagName: this.tagName, localName: this.localName, namespaceURI: this.namespaceURI, text, attributes, styles,
        parent: {tagName: parent.tagName, localName: parent.localName, namespaceURI: parent.namespaceURI,
          parentTagName: parent.parentNode?.tagName ?? null}},
      bbox: {x: box.x, y: box.y, width: box.width, height: box.height},
    });
    return box;
  };
}
"#;

const RAW_GEOMETRY_DRIVER_SUFFIX: &str = r#"
if (productionEntryPoint) {
  globalThis.__katanaMeasureText = function(...args) {
    fontMeasureCalls += 1;
    return originalMeasureText.apply(this, args);
  };
  globalThis.__katanaMeasureSvgTextMetrics = function(...args) {
    svgFontMetricCalls += 1;
    const metrics = originalMeasureSvgTextMetrics.apply(this, args);
    if (!recordAllSvgFontMetrics && !metricLabels.includes(String(args[0]))) return metrics;
    const result = {advance: metrics.advance, ascent: metrics.ascent, descent: metrics.descent};
    for (const name of ['layoutAscent', 'inkLeft', 'inkTop', 'inkRight', 'inkBottom']) {
      if (Object.hasOwn(metrics, name)) result[name] = metrics[name];
    }
    if (!recordAllSvgFontMetrics) {
      selectedMetricCalls += 1;
      const argsKey = JSON.stringify(args);
      const existing = svgFontMetricObservations.find((observation) => JSON.stringify(observation.args) === argsKey);
      if (existing) {
        selectedMetricDuplicateCalls += 1;
        if (JSON.stringify(existing.metrics) !== JSON.stringify(result))
          throw new Error('selected SVG font metric changed for identical arguments');
        return metrics;
      }
    }
    if (svgFontMetricObservations.length >= svgFontMetricTraceLimit)
      throw new Error('SVG font metric diagnostic trace limit exceeded');
    svgFontMetricObservations.push({ordinal: svgFontMetricObservations.length, args, metrics: result});
    return metrics;
  };
}
vendor.initialize = (actualConfig) => {
  initializeInput = JSON.parse(JSON.stringify(actualConfig));
  return originalInitialize(actualConfig);
};
let raw;
vendor.render = async (actualId, actualSource) => {
  renderCalls += 1; svgId = actualId; renderInput = actualSource;
  const result = await originalRender(actualId, actualSource);
  raw = result.svg;
  return result;
};
let processed;
if (productionEntryPoint) {
  processed = await katanaRunMermaidRuntime(request);
} else {
  globalThis.__katanaMermaidDiagramType = 'class';
  vendor.initialize(config);
  await vendor.render(request.svgId, request.source);
  processed = katanaNormalizeMermaidSvg(raw, request);
}
if (typeof raw !== 'string' || !raw) throw new Error('raw SVG absent');
return JSON.stringify({raw, processed, request, renderInput, svgId, initializeInput, expectedConfig: config,
  diagramType: globalThis.__katanaMermaidDiagramType, renderCalls, fontMeasureCalls, svgFontMetricCalls,
  svgMetricLabels: metricLabels, svgMetricObservationMode: recordAllSvgFontMetrics ? 'all' : 'selected',
  selectedMetricCalls, selectedMetricDuplicateCalls,
  svgFontMetricObservations, svgTextBoxObservations, c4GeometryObservations,
  c4GeometryObserverEnabled, c4GeometryHookCalls, c4GeometryHookNodes,
  c4GeometryPrototypeAfterRender: c4GeometryObserverEnabled ? {
    getBBox: c4GeometryPrototype?.getBBox === c4GetBBoxHook,
    getBoundingClientRect: c4GeometryPrototype?.getBoundingClientRect === c4GetBoundingClientRectHook,
  } : null});"#;

pub(super) fn fixture_mermaid_source(markdown: &str) -> Result<String, String> {
    let lines = markdown.lines().collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|line| line.trim_start().starts_with("~~~mermaid"))
        .ok_or("Mermaid fixture fence is missing or unclosed")?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.trim() == "~~~")
        .map(|(index, _)| index)
        .ok_or("Mermaid fixture fence is missing or unclosed")?;
    Ok(lines[start + 1..end].join("\n"))
}

fn envelope_svg<'a>(value: &'a serde_json::Value, name: &str) -> Result<&'a str, String> {
    value
        .get(name)
        .and_then(serde_json::Value::as_str)
        .filter(|svg| !svg.is_empty())
        .ok_or_else(|| format!("{name} SVG missing"))
}
