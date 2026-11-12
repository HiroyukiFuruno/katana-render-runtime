use crate::markdown::{
    color_preset::DiagramColorPreset,
    diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime},
    runtime_assets::RuntimeAsset,
};

#[path = "js_runtime_native_font_observer_data.rs"]
mod observation_data;

use observation_data::FontObservationEnvelope;

const NATIVE_FONT_OBSERVER: &str = r#"(() => {
  const limit = 512;
  const xHeightObservations = [];
  const svgMetricObservations = [];
  const bboxObservations = [];
  const innerHtmlObservations = [];
  globalThis.__katanaFontXHeightObservations = xHeightObservations;
  globalThis.__katanaSvgFontMetricObservations = svgMetricObservations;
  globalThis.__katanaSvgBBoxObservations = bboxObservations;
  globalThis.__katanaInnerHtmlObservations = innerHtmlObservations;
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
  const childSummary = (node, depth = 0, result = []) => {
    if (!node || depth > 3 || result.length === 16) return result;
    for (const child of node.children ?? []) {
      if (result.length === 16) break;
      result.push({
        localName: String(child.localName ?? '').slice(0, 24),
        namespaceURI: String(child.namespaceURI ?? '').slice(0, 96),
        childCount: Array.isArray(child.children) ? child.children.length : 0,
      });
      childSummary(child, depth + 1, result);
    }
    return result;
  };
  const installInnerHtmlObserver = () => {
    const prototype = globalThis.SVGElement?.prototype;
    const descriptor = prototype && Object.getOwnPropertyDescriptor(prototype, 'innerHTML');
    if (!descriptor?.set || !descriptor?.get) {
      throw new Error('native DOM innerHTML descriptor is required');
    }
    const originalSetter = descriptor.set;
    Object.defineProperty(prototype, 'innerHTML', {
      configurable: descriptor.configurable,
      enumerable: descriptor.enumerable,
      get: descriptor.get,
      set(value) {
        const input = String(value ?? '');
        const shouldRecord = /<(?:g|svg)\b/i.test(input);
        originalSetter.call(this, value);
        if (!shouldRecord) return;
        try {
          record(innerHtmlObservations, {
            targetLocalName: String(this.localName ?? '').slice(0, 24),
            inputPrefix: input.slice(0, 160),
            inputLength: input.length,
            inputHasSvg: /<svg\b/i.test(input),
            outputChildren: childSummary(this),
          });
        } catch (observerError) {
          void observerError;
        }
      },
    });
  };
  const installDomParserObserver = () => {
    const prototype = globalThis.DOMParser?.prototype;
    const original = prototype?.parseFromString;
    if (typeof original !== 'function') return;
    prototype.parseFromString = function(source, mimeType) {
      const input = String(source ?? '');
      const shouldRecord = /<(?:g|svg)\b/i.test(input);
      const result = original.call(this, source, mimeType);
      if (!shouldRecord) return result;
      try {
        record(innerHtmlObservations, {
          targetLocalName: `DOMParser:${String(mimeType ?? '').slice(0, 24)}`,
          inputPrefix: input.slice(0, 160),
          inputLength: input.length,
          inputHasSvg: /<svg\b/i.test(input),
          outputChildren: childSummary(result?.documentElement),
        });
      } catch (observerError) {
        void observerError;
      }
      return result;
    };
  };
  installInnerHtmlObserver();
  installDomParserObserver();
  let bboxObserverInstalled = false;
  const installBBoxObserver = () => {
    if (bboxObserverInstalled) return;
    const elementConstructor = globalThis.SVGElement;
    if (typeof elementConstructor?.prototype?.getBBox !== 'function') {
      throw new Error('native DOM getBBox bridge is required');
    }
    const original = elementConstructor.prototype.getBBox;
    elementConstructor.prototype.getBBox = function(...args) {
      const result = original.apply(this, args);
      try {
        const style = this.style;
        record(bboxObservations, {
          localName: String(this.localName ?? '').slice(0, 24),
          className: String(this.className ?? '').slice(0, 96),
          id: String(this.id ?? '').slice(0, 96),
          text: String(this.textContent ?? '').slice(0, 80),
          transformAttribute: String(this.getAttribute?.('transform') ?? '').slice(0, 160),
          styleAttribute: String(this.getAttribute?.('style') ?? '').slice(0, 256),
          displayAttribute: String(this.getAttribute?.('display') ?? '').slice(0, 24),
          visibilityAttribute: String(this.getAttribute?.('visibility') ?? '').slice(0, 24),
          inlineDisplay: String(style?.getPropertyValue?.('display') ?? '').slice(0, 24),
          inlineVisibility: String(style?.getPropertyValue?.('visibility') ?? '').slice(0, 24),
          x: Number.isFinite(result?.x) ? result.x : null,
          y: Number.isFinite(result?.y) ? result.y : null,
          width: Number.isFinite(result?.width) ? result.width : null,
          height: Number.isFinite(result?.height) ? result.height : null,
        });
      } catch (observerError) {
        void observerError;
      }
      return result;
    };
    bboxObserverInstalled = true;
  };
  globalThis.__katanaMeasureSvgFontXHeight = (...args) => {
    const result = xHeight(...args);
    const requested = font(args[0], args[1], args[2], args[3]);
    record(xHeightObservations, {...requested, xHeight: Number.isFinite(result) ? result : null});
    return result;
  };
  globalThis.__katanaMeasureSvgTextMetrics = (...args) => {
    installBBoxObserver();
    const result = textMetrics(...args);
    record(svgMetricObservations, {
      text: String(args[0]).slice(0, 80),
      ...font(args[1], args[2], args[3], args[4]),
      advance: Number.isFinite(result?.advance) ? result.advance : null,
      ascent: Number.isFinite(result?.ascent) ? result.ascent : null,
      descent: Number.isFinite(result?.descent) ? result.descent : null,
      inkLeft: Number.isFinite(result?.inkLeft) ? result.inkLeft : null,
      inkTop: Number.isFinite(result?.inkTop) ? result.inkTop : null,
      inkRight: Number.isFinite(result?.inkRight) ? result.inkRight : null,
      inkBottom: Number.isFinite(result?.inkBottom) ? result.inkBottom : null,
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
    /* WHY: buildは末尾に描画入口を必ず追加するため、観測用の入口に差し替える。 */
    drop(scripts.pop());
    scripts.push(observation_driver(&request_json));
    let envelope = DiagramV8Runtime::render(&scripts)?;
    let observed: FontObservationEnvelope =
        serde_json::from_str(&envelope).map_err(|error| error.to_string())?;
    let diagnostic =
        serde_json::to_string(&observed.observations()).map_err(|error| error.to_string())?;
    Ok((observed.svg, diagnostic))
}

fn observation_driver(request_json: &str) -> DiagramRuntimeScript<'static> {
    let driver = format!(
        "(async () => {{ const svg = await katanaRunMermaidRuntime({request_json}); return JSON.stringify({{svg, xHeightObservations: globalThis.__katanaFontXHeightObservations, svgMetricObservations: globalThis.__katanaSvgFontMetricObservations, bboxObservations: globalThis.__katanaSvgBBoxObservations, innerHtmlObservations: globalThis.__katanaInnerHtmlObservations}}); }})();"
    );
    DiagramRuntimeScript::owned("native-font-observer-result", driver)
}

#[cfg(test)]
mod tests {
    use super::{DiagramColorPreset, render_with_native_font_observations};

    #[test]
    fn native_font_observer_propagates_invalid_mermaid_source() -> Result<(), String> {
        let (svg, observations) = render_with_native_font_observations(
            "flowchart LR\n  A --> B",
            DiagramColorPreset::dark(),
        )?;
        assert!(svg.contains("<svg"));
        assert!(serde_json::from_str::<serde_json::Value>(&observations).is_ok());
        let error = render_with_native_font_observations(
            "flowchart LR\n  A -->",
            DiagramColorPreset::dark(),
        )
        .err()
        .ok_or("invalid flowchart unexpectedly rendered")?;
        assert!(!error.trim().is_empty());
        Ok(())
    }
}
