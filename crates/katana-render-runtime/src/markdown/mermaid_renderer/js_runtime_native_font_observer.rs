use crate::markdown::{
    color_preset::DiagramColorPreset,
    diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime},
    runtime_assets::RuntimeAsset,
};

const NATIVE_FONT_OBSERVER: &str = r#"(() => {
  const limit = 512;
  const xHeightObservations = [];
  const svgMetricObservations = [];
  globalThis.__katanaFontXHeightObservations = xHeightObservations;
  globalThis.__katanaSvgFontMetricObservations = svgMetricObservations;
  const xHeight = globalThis.__katanaMeasureSvgFontXHeight;
  const textMetrics = globalThis.__katanaMeasureSvgTextMetrics;
  if (typeof xHeight !== 'function' || typeof textMetrics !== 'function') {
    throw new Error('native font metric bridges are required');
  }
  const font = (family, size, weight, italic) => ({
    family: String(family).slice(0, 96),
    size: Number.isFinite(size) ? size : null,
    weight: Number.isFinite(weight) ? weight : null,
    italic: Boolean(italic),
  });
  const record = (collection, observation) => {
    if (collection.length === limit) collection.shift();
    collection.push(observation);
  };
  globalThis.__katanaMeasureSvgFontXHeight = (...args) => {
    const result = xHeight(...args);
    const requested = font(args[0], args[1], args[2], args[3]);
    record(xHeightObservations, {...requested, xHeight: Number.isFinite(result) ? result : null});
    return result;
  };
  globalThis.__katanaMeasureSvgTextMetrics = (...args) => {
    const result = textMetrics(...args);
    record(svgMetricObservations, {
      text: String(args[0]).slice(0, 80),
      ...font(args[1], args[2], args[3], args[4]),
      advance: Number.isFinite(result?.advance) ? result.advance : null,
      ascent: Number.isFinite(result?.ascent) ? result.ascent : null,
      inkTop: Number.isFinite(result?.inkTop) ? result.inkTop : null,
      layoutAscent: Number.isFinite(result?.layoutAscent) ? result.layoutAscent : null,
    });
    return result;
  };
})();"#;

pub(in super::super) fn render_with_native_font_observations(
    source: &str,
    preset: &DiagramColorPreset,
) -> Result<(String, String), String> {
    let asset = RuntimeAsset::mermaid();
    let path = asset.materialize_at(asset.materialized_path())?;
    let bundle = super::super::read_mermaid_bundle(&path)?;
    let request = super::super::MermaidRenderRequest::new(source, preset);
    let request_json = request.to_json_value().to_string();
    let mut scripts =
        super::super::MermaidRuntimeScripts::build(&bundle, &request_json, request.diagram_type);
    scripts.insert(
        1,
        DiagramRuntimeScript::borrowed("native-font-observer", NATIVE_FONT_OBSERVER),
    );
    let driver = format!(
        "(async () => {{ const svg = await katanaRunMermaidRuntime({request_json}); return JSON.stringify({{svg, xHeightObservations: globalThis.__katanaFontXHeightObservations, svgMetricObservations: globalThis.__katanaSvgFontMetricObservations}}); }})();"
    );
    let last = scripts
        .last_mut()
        .ok_or_else(|| "Mermaid runtime scripts missing".to_owned())?;
    *last = DiagramRuntimeScript::owned("native-font-observer-result", driver);
    let envelope = DiagramV8Runtime::render(&scripts)?;
    let observed: FontObservationEnvelope =
        serde_json::from_str(&envelope).map_err(|error| error.to_string())?;
    let diagnostic =
        serde_json::to_string(&observed.observations()).map_err(|error| error.to_string())?;
    Ok((observed.svg, diagnostic))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FontObservationEnvelope {
    svg: String,
    x_height_observations: Vec<XHeightObservation>,
    svg_metric_observations: Vec<SvgMetricObservation>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FontObservations<'a> {
    x_height: &'a [XHeightObservation],
    svg_metrics: &'a [SvgMetricObservation],
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct XHeightObservation {
    family: String,
    size: Option<f64>,
    weight: Option<f64>,
    italic: bool,
    x_height: Option<f64>,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SvgMetricObservation {
    text: String,
    family: String,
    size: Option<f64>,
    weight: Option<f64>,
    italic: bool,
    advance: Option<f64>,
    ascent: Option<f64>,
    ink_top: Option<f64>,
    layout_ascent: Option<f64>,
}

impl FontObservationEnvelope {
    fn observations(&self) -> FontObservations<'_> {
        FontObservations {
            x_height: &self.x_height_observations,
            svg_metrics: &self.svg_metric_observations,
        }
    }
}
