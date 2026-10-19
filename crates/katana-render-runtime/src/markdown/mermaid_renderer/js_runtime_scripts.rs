use super::diagram_type::MermaidDiagramType;
use crate::markdown::diagram_js_runtime::DiagramRuntimeScript;

pub(super) struct MermaidRuntimeScripts;

impl MermaidRuntimeScripts {
    pub(super) fn build<'a>(
        bundle: &'a str,
        request_json: &str,
        diagram_type: MermaidDiagramType,
    ) -> Vec<DiagramRuntimeScript<'a>> {
        Self::build_scripts(
            bundle,
            MERMAID_ZENUML_RUNTIME,
            request_json,
            diagram_type == MermaidDiagramType::Zenuml,
        )
    }

    fn build_scripts<'a>(
        bundle: &'a str,
        zenuml_bundle: &'a str,
        request_json: &str,
        uses_zenuml_runtime: bool,
    ) -> Vec<DiagramRuntimeScript<'a>> {
        let mut scripts = vec![
            DiagramRuntimeScript::borrowed("mermaid-runtime.min.js", MERMAID_RUNTIME),
            DiagramRuntimeScript::borrowed("mermaid.min.js", bundle),
            DiagramRuntimeScript::borrowed("mermaid-zenuml.min.js", zenuml_bundle),
        ];
        if uses_zenuml_runtime {
            scripts.push(DiagramRuntimeScript::borrowed(
                "zenuml-runtime.min.js",
                ZENUML_RUNTIME,
            ));
        }
        scripts.push(DiagramRuntimeScript::owned(
            "render-mermaid.js",
            render_script(request_json),
        ));
        scripts
    }

    #[cfg(test)]
    fn build_with_zenuml<'a>(
        bundle: &'a str,
        zenuml_bundle: &'a str,
        request_json: &str,
    ) -> Vec<DiagramRuntimeScript<'a>> {
        Self::build_scripts(
            bundle,
            zenuml_bundle,
            request_json,
            request_uses_zenuml_runtime(request_json),
        )
    }
}

fn render_script(request_json: &str) -> String {
    format!("katanaRunMermaidRuntime({request_json});")
}

#[cfg(test)]
fn request_uses_zenuml_runtime(request_json: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(request_json) else {
        return false;
    };
    if value
        .get("diagramType")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|diagram_type| diagram_type.eq_ignore_ascii_case("zenuml"))
    {
        return true;
    }
    let Some(source) = value.get("source").and_then(serde_json::Value::as_str) else {
        return false;
    };
    MermaidDiagramType::from_source(source) == MermaidDiagramType::Zenuml
}

const MERMAID_RUNTIME: &str = include_str!("../diagram_runtime/generated/mermaid-runtime.min.js");
const MERMAID_ZENUML_RUNTIME: &str =
    include_str!(concat!(env!("OUT_DIR"), "/mermaid-zenuml.min.js"));
const ZENUML_RUNTIME: &str = include_str!("../diagram_runtime/generated/zenuml-runtime.min.js");

#[cfg(test)]
#[path = "js_runtime_scripts_class_tests.rs"]
mod class_tests;
#[cfg(test)]
#[path = "js_runtime_scripts_entrypoint_tests.rs"]
mod entrypoint_tests;
#[cfg(test)]
#[path = "js_runtime_scripts_svg_tests.rs"]
mod svg_tests;

#[cfg(test)]
mod tests {
    use super::{MermaidDiagramType, MermaidRuntimeScripts};
    use crate::markdown::{
        diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime},
        runtime_assets::RuntimeAsset,
        svg_rasterize::SvgRasterizeOps,
    };

    fn assert_rendered_contains(rendered: &Result<String, String>, expected: &str) {
        assert!(
            rendered.as_ref().is_ok_and(|svg| svg.contains(expected)),
            "{rendered:?}"
        );
    }

    fn render_mermaid_request(
        bundle: &str,
        zenuml_bundle: &str,
        request_json: &str,
    ) -> Result<String, String> {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(bundle, zenuml_bundle, request_json);
        DiagramV8Runtime::render(&scripts)
    }

    #[test]
    fn build_includes_bundle_and_render_script() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml("bundle", "plugin", "{}");
        assert!(scripts.iter().any(|it| it.name == "mermaid-runtime.min.js"));
        assert!(scripts.iter().any(|it| it.name == "mermaid.min.js"));
        assert!(scripts.iter().any(|it| it.name == "mermaid-zenuml.min.js"));
        assert!(scripts.iter().any(|it| it.name == "render-mermaid.js"));
    }

    #[test]
    fn invalid_request_json_does_not_load_zenuml_runtime() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml("bundle", "plugin", "not-json");

        assert!(!scripts.iter().any(|it| it.name == "zenuml-runtime.min.js"));
    }

    #[test]
    fn zenuml_registration_runs_before_render() {
        let rendered = render_mermaid_request(
            fake_mermaid(),
            fake_zenuml(),
            r##"{"source":"zenuml\nA.method()","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"zenuml"}"##,
        );

        assert_rendered_contains(&rendered, "registered");
    }

    #[test]
    fn zenuml_directive_source_registers_external_diagram_without_request_hint() {
        let rendered = render_mermaid_request(
            fake_mermaid(),
            fake_zenuml(),
            r##"{"source":"%%{init: { \"theme\": \"dark\" }}%%\n%% comment\nzenuml\nA.method()","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        assert_rendered_contains(&rendered, "registered");
    }

    #[test]
    fn runtime_provides_constructable_style_sheet() {
        let rendered = render_mermaid_request(
            fake_mermaid_with_style_sheet(),
            "",
            r##"{"source":"graph TD; A-->B","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        assert_rendered_contains(&rendered, "font-family: sans-serif");
    }

    #[test]
    fn runtime_preserves_c4_description_with_explicit_line_break() {
        let scripts = MermaidRuntimeScripts::build(
            mermaid_test_bundle(),
            r##"{"source":"C4Context\nPerson(customer, \"顧客\", \"1行目<br/> 2行目\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
            MermaidDiagramType::Other,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(|svg| {
                svg.contains(">1行目</tspan>") && svg.contains(">2行目</tspan>")
            }),
            "{rendered:?}"
        );
    }

    fn mermaid_test_bundle() -> &'static str {
        must_utf8(std::str::from_utf8(must_asset_bytes(
            RuntimeAsset::mermaid().bytes(),
        )))
    }

    #[test]
    #[should_panic(expected = "unexpected test error: boom")]
    fn must_asset_bytes_reports_unexpected_test_errors() {
        let _ = must_asset_bytes(Err("boom".to_string()));
    }

    #[test]
    #[should_panic(expected = "invalid utf-8")]
    fn must_utf8_reports_unexpected_test_errors() {
        let _ = must_utf8(std::str::from_utf8(&[u8::MAX]));
    }

    fn must_asset_bytes(result: Result<&'static [u8], String>) -> &'static [u8] {
        match result {
            Ok(value) => value,
            Err(error) => fail(format!("unexpected test error: {error}")),
        }
    }

    fn must_utf8(result: Result<&'static str, std::str::Utf8Error>) -> &'static str {
        match result {
            Ok(value) => value,
            Err(error) => fail(format!("unexpected test error: {error}")),
        }
    }

    fn fail(message: String) -> ! {
        std::panic::resume_unwind(Box::new(message))
    }

    #[test]
    fn runtime_measures_horizontal_svg_arcs_like_a_browser() {
        let rendered = render_mermaid_request(
            fake_mermaid_measuring_horizontal_svg_arcs(),
            "",
            r##"{"source":"C4Container\nContainerDb(db, \"DB\", \"SQL\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
        );

        assert_rendered_contains(&rendered, "0:0:216:121.673");
    }

    #[test]
    fn runtime_measures_vertical_svg_arcs_like_a_browser() {
        let rendered = render_mermaid_request(
            fake_mermaid_measuring_vertical_svg_arcs(),
            "",
            r##"{"source":"C4Context\nSystemQueue(queue, \"Queue\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
        );

        assert_rendered_contains(
            &rendered,
            "-9.26887261930837:-73.650734:243.80661785792515:73.650734",
        );
    }

    #[test]
    fn runtime_measures_repeated_relative_curves_from_each_segment_origin() {
        let rendered = render_mermaid_request(
            fake_mermaid_measuring_repeated_relative_curves(),
            "",
            r##"{"source":"graph TD; A-->B","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        assert_rendered_contains(&rendered, "10:10:60:0");
    }

    #[test]
    fn runtime_matches_browser_c4_canvas_text_width() {
        let rendered = render_mermaid_request(
            fake_mermaid_measuring_c4_canvas_text(),
            "",
            r##"{"source":"C4Container\nContainerDb(db, \"DB\", \"SQL\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
        );

        assert_rendered_contains(&rendered, "54.279");
    }

    #[test]
    fn c4_wide_svg_width_preserves_native_raw_advance() -> Result<(), String> {
        assert_c4_native_width_contract(&[(
            "リレーショナルデータベーススキ",
            "Open Sans, sans-serif",
            10.5,
        )])
    }

    #[test]
    fn c4_relation_svg_widths_preserve_native_raw_advance() -> Result<(), String> {
        assert_c4_native_width_contract(&[
            ("Uses", "Open Sans, sans-serif", 12.0),
            ("[HTTPS]", "Open Sans, sans-serif", 12.0),
            ("Reads from and writes to", "Open Sans, sans-serif", 12.0),
            ("利用する", "Open Sans, sans-serif", 12.0),
            ("AVATAR", "Arial", 14.0),
            ("日本語の幅", "Hiragino Sans", 16.0),
        ])
    }

    const C4_NATIVE_WIDTH_PROBE: &str = r#"
globalThis.__katanaMermaidDiagramType = 'c4';
const calls = [];
const svgMeasure = globalThis.__katanaMeasureSvgTextMetrics;
const cssMeasure = globalThis.__katanaMeasureText;
globalThis.__katanaMeasureSvgTextMetrics = (...args) => {
  const metrics = svgMeasure(...args);
  calls.push({kind: 'svg', args, metrics});
  return metrics;
};
globalThis.__katanaMeasureText = (...args) => {
  const width = cssMeasure(...args);
  calls.push({kind: 'css', args, width});
  return width;
};
const rows = CASES.map(([text, family, size]) => {
  const firstCall = calls.length;
  const tspan = document.createElementNS('http://www.w3.org/2000/svg', 'tspan');
  tspan.style.setProperty('font-family', family);
  tspan.style.setProperty('font-size', `${size}px`);
  tspan.style.setProperty('font-weight', '400');
  tspan.style.setProperty('font-style', 'normal');
  tspan.textContent = text;
  const width = tspan.getComputedTextLength();
  const cssWidth = globalThis.__katanaMeasureText(text, family, size, 400, false);
  return {width, cssWidth, calls: calls.slice(firstCall)};
});
JSON.stringify(rows);
"#;

    fn assert_c4_native_width_contract(cases: &[(&str, &str, f32)]) -> Result<(), String> {
        let probe = c4_native_width_probe(cases)?;
        let scripts = [
            DiagramRuntimeScript::borrowed("mermaid-runtime.min.js", super::MERMAID_RUNTIME),
            DiagramRuntimeScript::owned("c4-native-width-contract.js", probe),
        ];
        SvgRasterizeOps::with_html_font_render_scope(|| {
            let rendered = DiagramV8Runtime::render(&scripts)?;
            let rows: Vec<serde_json::Value> =
                serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
            assert_c4_native_width_rows(cases, &rows)
        })
    }

    fn c4_native_width_probe(cases: &[(&str, &str, f32)]) -> Result<String, String> {
        serde_json::to_string(cases)
            .map(|cases| C4_NATIVE_WIDTH_PROBE.replace("CASES", &cases))
            .map_err(|error| error.to_string())
    }

    fn assert_c4_native_width_rows(
        cases: &[(&str, &str, f32)],
        rows: &[serde_json::Value],
    ) -> Result<(), String> {
        assert!(!cases.is_empty());
        assert_eq!(rows.len(), cases.len());
        let mut has_rounding_counterexample = false;
        let mut has_css_rounding_counterexample = false;
        for (row, &(text, family, size)) in rows.iter().zip(cases) {
            let (svg_counterexample, css_counterexample) =
                assert_c4_native_width_row(row, text, family, size)?;
            has_rounding_counterexample |= svg_counterexample;
            has_css_rounding_counterexample |= css_counterexample;
        }
        assert!(
            has_rounding_counterexample,
            "SVG inputs must distinguish raw advance from CSS ceil64"
        );
        assert!(
            has_css_rounding_counterexample,
            "CSS inputs must distinguish ceil64 from raw advance"
        );
        Ok(())
    }

    fn assert_c4_native_width_row(
        row: &serde_json::Value,
        text: &str,
        family: &str,
        size: f32,
    ) -> Result<(bool, bool), String> {
        let calls = row["calls"].as_array().ok_or("native calls missing")?;
        assert_eq!(calls.len(), 2, "{text:?}: {calls:?}");
        assert_c4_native_call_arguments(calls, text, family, size)?;
        let svg_advance = assert_c4_native_svg_reference(&calls[0], row, text, family, size)?;
        let css_raw = SvgRasterizeOps::measure_html_text(text, family, size, 400, false, 0.0, None)
            .map_err(|error| format!("CSS {text:?}, {family}: {error:?}"))?;
        assert!(css_raw.is_finite() && css_raw > 1.0);
        let css_expected = (f64::from(css_raw) * 64.0).ceil() / 64.0;
        assert_eq!(calls[1]["width"].as_f64(), Some(css_expected), "{text:?}");
        assert_eq!(row["cssWidth"].as_f64(), Some(css_expected), "{text:?}");
        let rounded_svg = (f64::from(svg_advance) * 64.0).ceil() / 64.0;
        Ok((
            rounded_svg as f32 != svg_advance,
            f64::from(css_raw) != css_expected,
        ))
    }

    fn assert_c4_native_svg_reference(
        call: &serde_json::Value,
        row: &serde_json::Value,
        text: &str,
        family: &str,
        size: f32,
    ) -> Result<f32, String> {
        let svg = SvgRasterizeOps::measure_svg_text_metrics(text, family, size, 400, false)
            .map_err(|error| format!("SVG {text:?}, {family}: {error:?}"))?;
        assert!(
            svg.advance.is_finite()
                && svg.advance > 1.0
                && svg.ascent.is_finite()
                && svg.descent.is_finite()
                && svg.ascent + svg.descent > 0.0,
            "nonzero native SVG reference required: {text:?}, {family}"
        );
        assert_c4_svg_metrics(call, row, &svg, text, family)?;
        Ok(svg.advance)
    }

    fn assert_c4_native_call_arguments(
        calls: &[serde_json::Value],
        text: &str,
        family: &str,
        size: f32,
    ) -> Result<(), String> {
        for (call, kind) in calls.iter().zip(["svg", "css"]) {
            assert_eq!(call["kind"].as_str(), Some(kind), "{text:?}");
            let args = call["args"].as_array().ok_or("arguments missing")?;
            assert_eq!(args.len(), 5, "{text:?}");
            assert_eq!(args[0].as_str(), Some(text));
            assert_eq!(args[1].as_str(), Some(family));
            assert_eq!(args[2].as_f64(), Some(f64::from(size)));
            assert_eq!(args[3].as_f64(), Some(400.0));
            assert_eq!(args[4].as_bool(), Some(false));
        }
        Ok(())
    }

    fn assert_c4_svg_metrics(
        call: &serde_json::Value,
        row: &serde_json::Value,
        expected: &crate::markdown::svg_rasterize::SvgTextRunMetrics,
        text: &str,
        family: &str,
    ) -> Result<(), String> {
        for (name, metric) in [
            ("advance", expected.advance),
            ("ascent", expected.ascent),
            ("descent", expected.descent),
        ] {
            assert_eq!(
                c4_native_f32(&call["metrics"][name])?,
                metric,
                "SVG {name}, {text:?}, {family}"
            );
        }
        /* WHY: callbackのraw値を維持し、DOM APIの幅だけChromeと同じ1/64pxへ切り上げる。 */
        let expected_dom_width = (f64::from(expected.advance) * 64.0).ceil() / 64.0;
        assert_eq!(row["width"].as_f64(), Some(expected_dom_width), "{text:?}");
        Ok(())
    }

    fn c4_native_f32(value: &serde_json::Value) -> Result<f32, String> {
        /* WHY: SVG APIのf32契約で厳密比較し、JSON復元差を許容誤差へ置き換えない。 */
        let decoded = value
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or("finite SVG number missing")?;
        let native = decoded as f32;
        native
            .is_finite()
            .then_some(native)
            .ok_or_else(|| "SVG number exceeds f32 range".into())
    }

    #[test]
    fn runtime_respects_nearest_c4_font_weight() {
        let rendered = render_mermaid_request(
            fake_mermaid_measuring_nearest_c4_font_weight(),
            "",
            r##"{"source":"C4Context\nSystemQueue(queue, \"Queue\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
        );

        assert_rendered_contains(&rendered, ">normal<");
    }

    #[test]
    fn runtime_applies_descendant_css_text_anchor_to_text_measurement() {
        let rendered = render_mermaid_request(
            fake_mermaid_with_descendant_text_anchor(),
            "",
            r##"{"source":"block-beta\nA[\"Markdown\"]","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        assert_rendered_contains(&rendered, "<text>-");
    }

    #[test]
    fn runtime_preserves_er_vendor_viewbox_and_geometry() -> Result<(), String> {
        let first = original_synthetic_svg(fake_mermaid_with_er_viewbox())?;
        let second = vary_synthetic_geometry(&first)?;
        assert_fake_vendor_svg_geometry_is_source_independent(
            [first, second],
            [
                "erDiagram\nDOCUMENT ||--o{ SECTION : contains",
                "erDiagram\nREPORT ||--o{ PAGE : contains",
            ],
        )
    }

    #[test]
    fn runtime_preserves_state_viewbox_and_label_geometry() -> Result<(), String> {
        let cases = [
            ("-8 -8 184.711 295", "184.711px", "translate(1, -9.5)", None),
            (
                "4 4 173 329",
                "173px",
                "translate(-60, -9.5)",
                Some("start"),
            ),
            ("4 0 173 333", "173px", "translate(13.5, 2)", Some("end")),
            ("0 0 173 333", "173px", "translate(0, -9.5)", Some("middle")),
        ];
        for (view_box, max_width, transform, text_anchor) in cases {
            assert_state_case(view_box, max_width, transform, text_anchor)?;
        }
        Ok(())
    }

    fn assert_state_case(
        view_box: &str,
        max_width: &str,
        transform: &str,
        text_anchor: Option<&str>,
    ) -> Result<(), String> {
        let anchor = match text_anchor {
            Some(value) => format!(r#" text-anchor="{value}""#),
            None => String::new(),
        };
        let raw_svg = format!(
            r#"<svg id="id" width="100%" style="max-width: {max_width};" viewBox="{view_box}" role="graphics-document document" aria-roledescription="stateDiagram"><g class="node statediagram-state "><g class="label" style="" transform="{transform}"><text y="-10.1" style=""{anchor}><tspan x="0">Pending</tspan></text></g></g></svg>"#
        );
        let rendered = render_fake_svg_for_source(&raw_svg, "stateDiagram-v2\n[*] --> Pending")?;
        assert_state_attributes_preserved(&rendered, view_box, max_width, transform)?;
        assert_state_anchor_preserved(&rendered, text_anchor)
    }

    fn assert_state_attributes_preserved(
        rendered: &str,
        view_box: &str,
        max_width: &str,
        transform: &str,
    ) -> Result<(), String> {
        for expected in [
            format!(r#"style="max-width: {max_width};""#),
            format!(r#"viewBox="{view_box}""#),
            format!(r#"transform="{transform}""#),
        ] {
            if !rendered.contains(&expected) {
                return Err(format!("state geometry was not preserved: {expected}"));
            }
        }
        Ok(())
    }

    fn assert_state_anchor_preserved(
        rendered: &str,
        text_anchor: Option<&str>,
    ) -> Result<(), String> {
        match text_anchor {
            Some(value) if rendered.contains(&format!(r#"text-anchor="{value}""#)) => Ok(()),
            Some(value) => Err(format!("state text anchor was not preserved: {value}")),
            None if !rendered.contains("text-anchor=") => Ok(()),
            None => Err("state text anchor was added".to_owned()),
        }
    }

    #[test]
    fn runtime_normalizes_requirement_viewbox_to_browser_bounds() -> Result<(), String> {
        let first = original_synthetic_svg(fake_mermaid_with_requirement_viewbox())?;
        let second = vary_synthetic_geometry(&first)?;
        assert_fake_svg_geometry_is_source_independent(
            [first, second],
            [
                "requirementDiagram\nrequirement test_req { id: 1 }",
                "requirementDiagram\nrequirement second_req { id: 2 }",
            ],
        )
    }

    #[test]
    fn runtime_preserves_fake_raw_class_geometry_and_generic_row_class() -> Result<(), String> {
        for (first, second, view_box, max_width) in [
            ("PreviewPane", "RenderedSection", "4 4 311.5 367", "311.5"),
            ("OtherPane", "OtherSection", "-7 -11 413.25 509.5", "413.25"),
        ] {
            let raw = independent_raw_class_geometry(first, second, view_box, max_width);
            let source =
                format!("classDiagram\nclass {first}\nclass {second}\n{first} --> {second}");
            let request = serde_json::json!({
                "source": source, "svgId": "id", "theme": "dark", "background": "#000",
                "fill": "#111", "text": "#fff", "stroke": "#fff", "arrow": "#fff",
            })
            .to_string();
            let fake = fake_mermaid_with_class_enumeration_layout(&raw)?;
            let scripts = MermaidRuntimeScripts::build_with_zenuml(&fake, "", &request);
            let rendered = DiagramV8Runtime::render(&scripts)?;
            /* WHY: 一般のrowクラス付与だけを許し、入力定義から独立した形状とpaintの全byteを保持する。 */
            let expected = raw.replace(
                "class=\"text-outer-tspan\"",
                "class=\"text-outer-tspan row\"",
            );
            assert_eq!(rendered, expected);
            assert!(!rendered.contains("235.1796875"));
            assert!(!rendered.contains("M117.59,146L117.59,150.167"));
        }
        Ok(())
    }

    #[test]
    fn runtime_normalizes_mindmap_fixture_layout_to_browser_bounds() -> Result<(), String> {
        let source = "mindmap\n  root((mindmap))";
        let first = original_mindmap_fixture_svg(0);
        assert_eq!(svg_attribute_value(&first, "x"), None);
        let with_x = first.replacen("<path ", "<path x=\"17\" ", 1);
        assert_eq!(svg_attribute_value(&with_x, "x"), Some("17"));
        assert_eq!(
            render_fake_mermaid_for_source(fake_mermaid_with_mindmap_layout(), source)?,
            render_fake_svg_for_source(&first, source)?,
        );
        let normalized = render_fake_svg_for_source(&first, source)?;
        let namespaced =
            normalized.replacen("<svg ", "<svg xmlns=\"http://www.w3.org/2000/svg\" ", 1);
        assert!(matches!(
            assert_synthetic_svg_contract(&normalized, &namespaced),
            Err(error) if error.contains("root namespace")
        ));
        let second = original_mindmap_fixture_svg(1);
        assert_fake_svg_geometry_is_source_independent(
            [first, second],
            [source, "mindmap\n  root((mindmap revised))"],
        )
    }

    #[test]
    fn runtime_normalizes_simple_mindmap_fixture_layout_to_browser_bounds() -> Result<(), String> {
        let source = "mindmap\n  root((Mermaid))";
        let first = original_simple_mindmap_fixture_svg(0);
        assert_eq!(
            render_fake_mermaid_for_source(fake_mermaid_with_simple_mindmap_layout(), source)?,
            render_fake_svg_for_source(&first, source)?,
        );
        let second = original_simple_mindmap_fixture_svg(1);
        assert_fake_svg_geometry_is_source_independent(
            [first, second],
            [source, "mindmap\n  root((Mermaid revised))"],
        )
    }

    #[test]
    fn runtime_normalizes_localized_requirement_layout_to_browser_bounds() -> Result<(), String> {
        let first = original_synthetic_svg(fake_mermaid_with_localized_requirement_layout())?;
        let second = vary_synthetic_geometry(&first)?;
        assert_fake_svg_geometry_is_source_independent(
            [first, second],
            [
                "requirementDiagram\nrequirement テスト要件 { id: 1 }",
                "requirementDiagram\nrequirement   テスト要件 { id: 2 }",
            ],
        )
    }

    #[test]
    fn runtime_normalizes_japanese_sequence_activation_layout_to_browser_bounds()
    -> Result<(), String> {
        let first =
            original_synthetic_svg(fake_mermaid_with_japanese_sequence_activation_layout())?;
        let second = vary_synthetic_geometry(&first)?;
        assert_fake_svg_geometry_is_source_independent(
            [first, second],
            [
                "sequenceDiagram\n田中->>+鈴木: こんにちは鈴木さん、お元気ですか？",
                "sequenceDiagram\n佐藤->>+高橋: 今日はよろしくお願いします",
            ],
        )
    }

    #[test]
    fn runtime_normalizes_english_sequence_activation_layout_to_browser_bounds()
    -> Result<(), String> {
        let first = original_synthetic_svg(fake_mermaid_with_english_sequence_activation_layout())?;
        let second = vary_synthetic_geometry(&first)?;
        assert_fake_svg_geometry_is_source_independent(
            [first, second],
            [
                "sequenceDiagram\nAlice->>+John: Hello John, how are you?",
                "sequenceDiagram\nMary->>+Paul: Welcome, how are you?",
            ],
        )
    }

    #[test]
    fn runtime_normalizes_c4_review_keeps_svg_when_source_does_not_match() {
        let rendered = render_mermaid_request(
            fake_mermaid_with_c4_review_svg(),
            "",
            r##"{"source":"graph TD\nA-->B","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains(r#"style="max-width: 300px;""#)
                    && it.contains(r#"viewBox="0 0 300 200""#)
                    && !it.contains(r#"viewBox="0 -70 1148 441""#)),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_normalizes_c4_review_changes_svg_when_source_matches() -> Result<(), String> {
        let first = original_synthetic_svg(fake_mermaid_with_c4_review_svg())?;
        let second = vary_synthetic_geometry(&first)?;
        assert_fake_svg_geometry_is_source_independent(
            [first, second],
            [
                "C4Context\nPerson(a,\"A\")",
                "C4Context \n Person(b, \"B\")",
            ],
        )
    }

    #[test]
    fn runtime_reads_html_line_breaks_as_visible_text() {
        let rendered = render_mermaid_request(
            fake_mermaid_with_html_line_break(),
            "",
            r##"{"source":"mindmap\n  root((mindmap))","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        assert_rendered_contains(&rendered, "On effectiveness\nand features");
    }

    #[test]
    fn runtime_measures_svg_without_viewbox_with_dom_metrics_helper() {
        let rendered = render_mermaid_request(
            fake_mermaid_measuring_svg_without_viewbox(),
            "",
            r##"{"source":"graph TD; A-->B","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        assert_rendered_contains(&rendered, "800:600");
    }

    fn fake_mermaid() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  registerExternalDiagrams: async (diagrams) => {
    globalThis.__registeredDiagram = diagrams[0].id;
  },
  render: async (id) => {
    const text = globalThis.__registeredDiagram ?? "missing";
    return { svg: `<svg id="${id}"><text>${text}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_svg_without_viewbox() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    globalThis.innerWidth = 800;
    globalThis.innerHeight = 600;
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    document.body.appendChild(svg);
    const box = svg.getBoundingClientRect();
    svg.remove();
    return { svg: `<svg id="${id}" viewBox="0 0 800 600"><text>${box.width}:${box.height}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_horizontal_svg_arcs() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", "M0,15.8358 a108,15.8358 0,0,0 216,0 a108,15.8358 0,0,0 -216,0 l0,90.0019 a108,15.8358 0,0,0 216,0 l0,-90.0019");
    const box = path.getBBox();
    return { svg: `<svg id="${id}"><text>${box.x}:${box.y}:${box.width}:${box.height}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_vertical_svg_arcs() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", "M0,0 a9.26887261930837,36.825367 0,0,1 0,-73.650734 l225.26887261930838,0 a9.26887261930837,36.825367 0,0,1 0,73.650734 M225.26887261930838,-73.650734 a9.26887261930837,36.825367 0,0,0 0,73.650734 l-225.26887261930838,0");
    const box = path.getBBox();
    return { svg: `<svg id="${id}"><text>${box.x}:${box.y}:${box.width}:${box.height}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_repeated_relative_curves() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", "M 10 10 c 10 0 20 0 30 0 c 10 0 20 0 30 0");
    const box = path.getBBox();
    return { svg: `<svg id="${id}"><text>${box.x}:${box.y}:${box.width}:${box.height}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_c4_canvas_text() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const context = document.createElement("canvas").getContext("2d");
    context.font = "10.5px Open Sans, sans-serif";
    const width = context.measureText("[Container: ").width.toFixed(3);
    return { svg: `<svg id="${id}"><text>${width}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_nearest_c4_font_weight() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
    group.style.setProperty("font-weight", "bold");
    const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
    const outer = document.createElementNS("http://www.w3.org/2000/svg", "tspan");
    const inner = document.createElementNS("http://www.w3.org/2000/svg", "tspan");
    outer.style.setProperty("font-size", "14px");
    outer.style.setProperty("font-family", "Open Sans, sans-serif");
    inner.setAttribute("font-weight", "normal");
    inner.textContent = "Banking System F Queue";
    outer.appendChild(inner);
    text.appendChild(outer);
    group.appendChild(text);
    const weight = outer.getComputedTextLength() < 165 ? "normal" : "bold";
    return { svg: `<svg id="${id}"><text>${weight}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_with_style_sheet() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const sheet = new CSSStyleSheet();
    const index = sheet.insertRule(".style-built { fill: red; }", sheet.cssRules.length);
    sheet.cssRules[index].style.setProperty("font-family", "sans-serif");
    if (sheet.cssRules[index].style.length !== 2) throw new Error("missing CSS declarations");
    const theme = new CSSStyleSheet();
    theme.replaceSync(".theme-built { stroke: blue; }");
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    const tspan = document.createElementNS("http://www.w3.org/2000/svg", "tspan");
    tspan.setAttribute("class", "style-built");
    svg.appendChild(tspan);
    if (svg.querySelectorAll("tspan.style-built").length !== 1) throw new Error("compound selector failed");
    const css = [...sheet.cssRules, ...theme.cssRules].map((rule) => rule.cssText).join("\n");
    return { svg: `<svg id="${id}"><style>${css}</style><text class="style-built">ok</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_with_descendant_text_anchor() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("id", id);
    const style = document.createElementNS("http://www.w3.org/2000/svg", "style");
    style.textContent = `#${id} .flowchart-label text { text-anchor: middle; }`;
    svg.appendChild(style);
    const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
    group.setAttribute("class", "flowchart-label");
    const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
    const tspan = document.createElementNS("http://www.w3.org/2000/svg", "tspan");
    tspan.setAttribute("x", "0");
    tspan.textContent = "Markdown";
    text.appendChild(tspan);
    group.appendChild(text);
    svg.appendChild(group);
    document.body.appendChild(svg);
    const box = text.getBBox();
    svg.remove();
    return { svg: `<svg id="${id}"><text>${box.x}:${box.width}</text></svg>` };
  }
};
"#
    }

    type SyntheticViewBox = [f64; 4];
    type SyntheticRootAttributes = std::collections::BTreeMap<String, String>;
    const SYNTHETIC_VARIANT_WIDTH: f64 = 37.0;
    const SYNTHETIC_VARIANT_HEIGHT: f64 = 23.0;
    const SYNTHETIC_VARIANT_X: f64 = 7.0;
    const SYNTHETIC_VARIANT_Y: f64 = 11.0;
    const SVG_NUMBER_PRECISION: f64 = 1_000_000.0;
    const NORMALIZED_ORIGIN: f64 = -8.0;
    const ORIGIN_MATCH_EPSILON: f64 = 0.01;

    fn assert_fake_svg_geometry_is_source_independent(
        geometries: [String; 2],
        source_variants: [&str; 2],
    ) -> Result<(), String> {
        assert_fake_svg_geometry_is_source_independent_with_contract(
            geometries,
            source_variants,
            false,
        )
    }

    fn assert_fake_vendor_svg_geometry_is_source_independent(
        geometries: [String; 2],
        source_variants: [&str; 2],
    ) -> Result<(), String> {
        assert_fake_svg_geometry_is_source_independent_with_contract(
            geometries,
            source_variants,
            true,
        )
    }

    fn assert_fake_svg_geometry_is_source_independent_with_contract(
        geometries: [String; 2],
        source_variants: [&str; 2],
        preserve_vendor_attributes: bool,
    ) -> Result<(), String> {
        let mut outputs = Vec::new();
        let mut output_boxes = Vec::new();
        for raw in geometries {
            let first = render_fake_svg_for_source(&raw, source_variants[0])?;
            let second = render_fake_svg_for_source(&raw, source_variants[1])?;
            assert_eq!(first, second, "source changed the same raw SVG");
            if preserve_vendor_attributes {
                assert_synthetic_vendor_svg_contract(&raw, &first)?;
            } else {
                assert_synthetic_svg_contract(&raw, &first)?;
            }
            output_boxes.push(
                svg_attribute_value(&first, "viewBox")
                    .ok_or("missing viewBox")?
                    .to_owned(),
            );
            outputs.push(first);
        }
        assert_ne!(
            output_boxes[0], output_boxes[1],
            "distinct geometry collapsed"
        );
        assert_ne!(outputs[0], outputs[1]);
        Ok(())
    }

    fn assert_synthetic_svg_contract(raw: &str, rendered: &str) -> Result<(), String> {
        assert_synthetic_root_namespace_is_preserved(raw, rendered)?;
        let actual = synthetic_root_attributes(rendered)?;
        assert_eq!(
            actual,
            expected_synthetic_root_attributes(raw)?,
            "root attribute contract"
        );
        assert_eq!(
            synthetic_svg_body(raw)?,
            synthetic_svg_body(rendered)?,
            "unclassified child geometry, attributes, labels and structure must remain byte-exact"
        );
        Ok(())
    }

    fn assert_synthetic_vendor_svg_contract(raw: &str, rendered: &str) -> Result<(), String> {
        assert_synthetic_root_namespace_is_preserved(raw, rendered)?;
        assert_eq!(
            synthetic_root_attributes(raw)?,
            synthetic_root_attributes(rendered)?,
            "vendor root attributes changed"
        );
        assert_eq!(
            synthetic_svg_body(raw)?,
            synthetic_svg_body(rendered)?,
            "vendor child geometry, attributes, labels and structure changed"
        );
        Ok(())
    }

    fn assert_synthetic_root_namespace_is_preserved(
        raw: &str,
        rendered: &str,
    ) -> Result<(), String> {
        let raw_document =
            resvg::usvg::roxmltree::Document::parse(raw).map_err(|error| error.to_string())?;
        let rendered_document =
            resvg::usvg::roxmltree::Document::parse(rendered).map_err(|error| error.to_string())?;
        let raw_namespace = raw_document
            .root_element()
            .tag_name()
            .namespace()
            .map(str::to_owned);
        let rendered_namespace = rendered_document
            .root_element()
            .tag_name()
            .namespace()
            .map(str::to_owned);
        if raw_namespace != rendered_namespace {
            return Err(format!(
                "root namespace must remain identical: {raw_namespace:?} != {rendered_namespace:?}"
            ));
        }
        Ok(())
    }

    fn synthetic_svg_body(svg: &str) -> Result<&str, String> {
        let open = svg.find("<svg").ok_or("missing svg root")?;
        let start = open + svg[open..].find('>').ok_or("missing root end")? + 1;
        let end = svg.rfind("</svg>").ok_or("missing svg close")?;
        svg.get(start..end).ok_or("invalid SVG body range".into())
    }

    fn synthetic_root_attributes(svg: &str) -> Result<SyntheticRootAttributes, String> {
        let document =
            resvg::usvg::roxmltree::Document::parse(svg).map_err(|error| error.to_string())?;
        let root = document.root_element();
        if root.tag_name().name() != "svg"
            || root
                .tag_name()
                .namespace()
                .is_some_and(|namespace| namespace != "http://www.w3.org/2000/svg")
        {
            return Err("root must be SVG".into());
        }
        Ok(root
            .attributes()
            .map(|attribute| {
                let name = attribute.namespace().map_or_else(
                    || attribute.name().to_owned(),
                    |namespace| format!("{{{namespace}}}{}", attribute.name()),
                );
                (name, attribute.value().to_owned())
            })
            .collect())
    }

    fn expected_synthetic_root_attributes(raw: &str) -> Result<SyntheticRootAttributes, String> {
        let mut attributes = synthetic_root_attributes(raw)?;
        let input = parse_view_box(attributes.get("viewBox").ok_or("missing input viewBox")?)?;
        let (expected, resize) = expected_synthetic_view_box(raw, input)?;
        if expected != input {
            attributes.insert(
                "viewBox".into(),
                expected.map(format_synthetic_number).join(" "),
            );
        }
        if resize {
            resize_synthetic_root_attributes(&mut attributes, expected)?;
        }
        Ok(attributes)
    }

    fn expected_synthetic_view_box(
        raw: &str,
        input: SyntheticViewBox,
    ) -> Result<(SyntheticViewBox, bool), String> {
        let role =
            svg_attribute_value(raw, "aria-roledescription").ok_or("missing diagram role")?;
        let negative = (input[0] - NORMALIZED_ORIGIN).abs() < ORIGIN_MATCH_EPSILON
            && (input[1] - NORMALIZED_ORIGIN).abs() < ORIGIN_MATCH_EPSILON;
        match (role, negative) {
            ("requirement", true) => Ok((
                [
                    0.0,
                    0.0,
                    rounded_synthetic_number(input[2] + input[0]),
                    rounded_synthetic_number(input[3] + input[1]),
                ],
                true,
            )),
            _ => Ok((input, false)),
        }
    }

    fn resize_synthetic_root_attributes(
        attributes: &mut SyntheticRootAttributes,
        view_box: SyntheticViewBox,
    ) -> Result<(), String> {
        let width = view_box[2].ceil().max(1.0).to_string();
        attributes.insert("width".into(), width.clone());
        attributes.insert("height".into(), view_box[3].ceil().max(1.0).to_string());
        if let Some(style) = attributes.get_mut("style") {
            if !style.starts_with("max-width: ") || style.matches(';').count() != 1 {
                return Err("fixture style is not an isolated max-width property".into());
            }
            *style = format!("max-width: {width}px;");
        }
        Ok(())
    }

    fn original_synthetic_svg(fake: &str) -> Result<String, String> {
        let raw = fake
            .split_once("return { svg: `")
            .ok_or("original fixture has no SVG template")?
            .1;
        let raw = raw
            .split_once("` };")
            .ok_or("original SVG template end missing")?
            .0
            .replace("${id}", "id");
        if raw.contains("${") {
            return Err("original fixture has an unsupported interpolation".into());
        }
        Ok(raw)
    }

    fn vary_synthetic_geometry(raw: &str) -> Result<String, String> {
        let view_box = svg_attribute_value(raw, "viewBox").ok_or("missing input viewBox")?;
        let mut values = parse_view_box(view_box)?;
        values[2] += SYNTHETIC_VARIANT_WIDTH;
        values[3] += SYNTHETIC_VARIANT_HEIGHT;
        let changed = raw.replacen(
            &format!("viewBox=\"{view_box}\""),
            &format!(
                "viewBox=\"{}\"",
                values.map(format_synthetic_number).join(" ")
            ),
            1,
        );
        vary_synthetic_child_geometry(changed)
    }

    fn vary_synthetic_child_geometry(raw: String) -> Result<String, String> {
        if let Some(transform) = svg_attribute_value(&raw, "transform") {
            return vary_synthetic_translate(&raw, transform);
        }
        if let Some(x) = svg_attribute_value(&raw, "x") {
            return vary_synthetic_x(&raw, x);
        }
        if !raw.contains("<g>") {
            return Err("fixture has no mutable child geometry".into());
        }
        Ok(raw.replacen(
            "<g>",
            &format!("<g transform=\"translate({SYNTHETIC_VARIANT_X}, {SYNTHETIC_VARIANT_Y})\">"),
            1,
        ))
    }

    fn vary_synthetic_translate(raw: &str, transform: &str) -> Result<String, String> {
        let origin = parse_synthetic_translate(transform)?;
        let changed = format!(
            "translate({}, {})",
            format_synthetic_number(origin[0] + SYNTHETIC_VARIANT_X),
            format_synthetic_number(origin[1] + SYNTHETIC_VARIANT_Y)
        );
        Ok(raw.replacen(
            &format!("transform=\"{transform}\""),
            &format!("transform=\"{changed}\""),
            1,
        ))
    }

    fn vary_synthetic_x(raw: &str, x: &str) -> Result<String, String> {
        let value = x.parse::<f64>().map_err(|error| error.to_string())?;
        Ok(raw.replacen(
            &format!(" x=\"{x}\""),
            &format!(
                " x=\"{}\"",
                format_synthetic_number(value + SYNTHETIC_VARIANT_X)
            ),
            1,
        ))
    }

    fn parse_synthetic_translate(value: &str) -> Result<[f64; 2], String> {
        let numbers = value
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .ok_or("expected translate transform")?
            .split(',')
            .map(|value| {
                value
                    .trim()
                    .parse::<f64>()
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        numbers
            .try_into()
            .map_err(|_| "expected two translate components".into())
    }

    fn parse_view_box(value: &str) -> Result<SyntheticViewBox, String> {
        let values = value
            .split_whitespace()
            .map(|value| value.parse::<f64>().map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        if !values.iter().all(|value| value.is_finite()) {
            return Err("non-finite viewBox".into());
        }
        values
            .try_into()
            .map_err(|_| "expected four viewBox components".into())
    }

    fn rounded_synthetic_number(value: f64) -> f64 {
        (value * SVG_NUMBER_PRECISION).round() / SVG_NUMBER_PRECISION
    }

    fn format_synthetic_number(value: f64) -> String {
        rounded_synthetic_number(value).to_string()
    }

    fn original_mindmap_fixture_svg(variant: usize) -> String {
        let labels = [
            "mindmap", "Origins", "Research", "Tools", "item-0", "item-1", "item-2", "item-3",
            "item-4", "item-5", "item-6", "item-7", "item-8", "item-9", "item-10", "item-11",
            "item-12", "item-13", "item-14", "item-15", "wrapped",
        ];
        let label_markup = labels
            .iter()
            .map(|label| format!("<text>{label}</text>"))
            .collect::<String>();
        let (view_box, max_width, transform, paths) = if variant == 0 {
            ("0 0 900 900", "900px", "1, 2", ["M0", "M1", "M2", "M3"])
        } else {
            ("0 0 720 840", "720px", "4, 6", ["M10", "M11", "M12", "M13"])
        };
        format!(
            "<svg id=\"id\" width=\"100%\" class=\"mindmap\" style=\"max-width: {max_width};\" viewBox=\"{view_box}\" role=\"graphics-document document\" aria-roledescription=\"mindmap\"><g transform=\"translate({transform})\"></g><path d=\"{}\"></path><path d=\"{}\"></path><path d=\"{}\"></path><path d=\"{}\"></path>{label_markup}</svg>",
            paths[0], paths[1], paths[2], paths[3]
        )
    }

    fn original_simple_mindmap_fixture_svg(variant: usize) -> String {
        let labels = ["Mermaid", "Runtime", "Rasterize", "Quality"];
        let label_markup = labels
            .iter()
            .map(|label| format!("<text>{label}</text>"))
            .collect::<String>();
        let (view_box, max_width, transform, paths) = if variant == 0 {
            ("0 0 900 900", "900px", "1, 2", ["M0", "M1", "M2", "M3"])
        } else {
            ("0 0 760 880", "760px", "7, 9", ["M10", "M11", "M12", "M13"])
        };
        format!(
            "<svg id=\"id\" width=\"100%\" class=\"mindmap\" style=\"max-width: {max_width};\" viewBox=\"{view_box}\" role=\"graphics-document document\" aria-roledescription=\"mindmap\"><g transform=\"translate({transform})\"></g><path d=\"{}\"></path><path d=\"{}\"></path><path d=\"{}\"></path><path d=\"{}\"></path>{label_markup}</svg>",
            paths[0], paths[1], paths[2], paths[3]
        )
    }

    fn render_fake_svg_for_source(raw_svg: &str, source: &str) -> Result<String, String> {
        let serialized_svg = serde_json::to_string(raw_svg).map_err(|error| error.to_string())?;
        let fake_mermaid = format!(
            "globalThis.mermaid = {{initialize() {{}}, render: async () => ({{svg: {serialized_svg}}})}};",
        );
        render_fake_mermaid_for_source(&fake_mermaid, source)
    }

    fn render_fake_mermaid_for_source(fake_mermaid: &str, source: &str) -> Result<String, String> {
        let request = serde_json::json!({
            "source": source, "svgId": "id", "theme": "dark", "background": "#000",
            "fill": "#111", "text": "#fff", "stroke": "#fff", "arrow": "#fff",
        })
        .to_string();
        let scripts = MermaidRuntimeScripts::build_with_zenuml(fake_mermaid, "", &request);
        DiagramV8Runtime::render(&scripts)
    }

    fn svg_attribute_value<'a>(svg: &'a str, name: &str) -> Option<&'a str> {
        let attribute = format!(" {name}=\"");
        svg.split_once(&attribute)?.1.split('\"').next()
    }

    fn fake_mermaid_with_er_viewbox() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="100%" class="erDiagram" style="max-width: 164.525px;" viewBox="-8 -8 164.525 535.5" role="graphics-document document" aria-roledescription="er"><g><g class="root"><g class="nodes"><g class="node default " id="${id}-entity-DOCUMENT-0" transform="translate(78.2625, 64.625)"><g class="outer-path" style=""><path d="M-64.9805 -56.625 L64.9805 -56.625 L64.9805 56.625 L-64.9805 56.625"></path></g></g><g class="node default " id="${id}-entity-SECTION-1" transform="translate(78.2625, 280.875)"><g class="outer-path" style=""><path d="M-70.2625 -56.625 L70.2625 -56.625 L70.2625 56.625 L-70.2625 56.625"></path></g></g><g class="node default " id="${id}-entity-DIAGRAM-2" transform="translate(78.2625, 480)"><rect class="basic label-container" x="-52.3135" y="-39.5" width="104.627" height="79"></rect></g></g></g></g></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_requirement_viewbox() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="100%" class="requirementDiagram" style="max-width: 181.737px;" viewBox="-8 -8 181.737 390" role="graphics-document document" aria-roledescription="requirement"><g class="label" data-id="edge" transform="translate(0, -9.5)"><rect class="background" style="" x="-47.75500000000001" y="-2" width="95.51000000000002" height="23"></rect></g></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_localized_requirement_layout() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="162" class="requirementDiagram" style="max-width: 162px;" viewBox="0 0 161.212 382" role="graphics-document document" aria-roledescription="requirement"><g class="node default " transform="translate(80.60600000000001, 62.5)"><path d="M-69.0515 -54.5 L69.0515 -54.5 L69.0515 54.5 L-69.0515 54.5"></path><g class="label" transform="translate(-20.486517499999998, -22)"><text>テスト要件</text></g></g></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_class_enumeration_layout(raw: &str) -> Result<String, String> {
        let serialized = serde_json::to_string(raw).map_err(|error| error.to_string())?;
        Ok(format!(
            "globalThis.mermaid = {{initialize() {{}}, render: async () => ({{svg: {serialized}}})}};",
        ))
    }

    fn independent_raw_class_geometry(
        first: &str,
        second: &str,
        view_box: &str,
        max_width: &str,
    ) -> String {
        let paths: String = (0..15)
            .map(|index| {
                format!(
                    "<path d=\"M{} 19 L{} 27\" stroke=\"#abcdef\"/>",
                    index + 3,
                    index + 11
                )
            })
            .collect();
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" id=\"id\" width=\"100%\" class=\"classDiagram\" style=\"max-width: {max_width}px;\" viewBox=\"{view_box}\" role=\"graphics-document document\" aria-roledescription=\"classDiagram\"><g class=\"root\" transform=\"translate(13.25, 17.75)\"><g class=\"node default \" id=\"classId-{first}-0\" transform=\"translate(41.5, 67.25)\"><g transform=\"translate(8.5, -3.25)\"><text class=\"text-outer-tspan\" x=\"9.75\" y=\"16.5\" fill=\"#123456\">{first} {second} «enumeration»</text></g><g class=\"methods-group text\" transform=\"translate(1.5, 2.75)\"><text>+method()</text></g></g>{paths}</g></svg>",
        )
    }

    fn fake_mermaid_with_mindmap_layout() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const labels = ["mindmap", "Origins", "Research", "Tools", ...Array.from({ length: 16 }, (_, index) => `item-${index}`), "wrapped"];
    const texts = labels.map((label) => `<text>${label}</text>`).join("");
    return { svg: `<svg id="${id}" width="100%" class="mindmap" style="max-width: 900px;" viewBox="0 0 900 900" role="graphics-document document" aria-roledescription="mindmap"><g transform="translate(1, 2)"></g><path d="M0"></path><path d="M1"></path><path d="M2"></path><path d="M3"></path>${texts}</svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_simple_mindmap_layout() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="100%" class="mindmap" style="max-width: 900px;" viewBox="0 0 900 900" role="graphics-document document" aria-roledescription="mindmap"><g transform="translate(1, 2)"></g><path d="M0"></path><path d="M1"></path><path d="M2"></path><path d="M3"></path><text>Mermaid</text><text>Runtime</text><text>Rasterize</text><text>Quality</text></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_japanese_sequence_activation_layout() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="595" height="355" viewBox="-50 -10 595 355" role="graphics-document document" aria-roledescription="sequence"><g><rect x="415" y="113" width="10" height="144" class="activation0"></rect></g><g><rect x="420" y="163" width="10" height="46" class="activation1"></rect></g><text x="244" y="80" class="messageText">こんにちは鈴木さん、お元気ですか？</text><line x1="76" y1="111" x2="412" y2="111" data-et="message"></line><text x="247" y="172" class="messageText">こんにちは田中さん、聞こえますよ！</text><line x1="415" y1="205" x2="79" y2="205" data-et="message"></line><line x1="415" y1="253" x2="79" y2="253" data-et="message"></line></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_english_sequence_activation_layout() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="500" height="355" viewBox="-50 -10 500 355" role="graphics-document document" aria-roledescription="sequence"><g><rect x="250" y="269" width="150" height="65" class="actor actor-bottom"></rect><line id="actor1" x1="325" y1="65" x2="325" y2="269"></line><rect x="250" y="0" width="150" height="65" class="actor actor-top"></rect></g><g><rect x="320" y="113" width="10" height="144" class="activation0"></rect></g><g><rect x="325" y="163" width="10" height="46" class="activation1"></rect></g><text x="197" y="80" class="messageText">Hello John, how are you?</text><line x1="76" y1="111" x2="317" y2="111" data-et="message"></line><text x="200" y="172" class="messageText">Hi Alice, I can hear you!</text><line x1="320" y1="205" x2="79" y2="205" data-et="message"></line><line x1="320" y1="253" x2="79" y2="253" data-et="message"></line></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_c4_review_svg() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="100%" style="max-width: 300px;" viewBox="0 0 300 200" role="graphics-document document" aria-roledescription="c4"><g><text>C4 Review</text></g></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_html_line_break() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const node = document.createElement("div");
    node.innerHTML = "On effectiveness<br/>and features";
    return { svg: `<svg id="${id}"><text>${node.innerText}</text></svg>` };
  }
};
"#
    }

    fn fake_zenuml() -> &'static str {
        r#"globalThis["mermaid-zenuml"] = { id: "registered" };"#
    }
    const FIXTURE_SENSITIVE_GEOMETRY_CASES: [(&str, [&str; 2]); 7] = [
        (
            r#"<svg id="id" width="300" height="120" style="max-width: 300px;" viewBox="-20 -10 340 120" role="graphics-document document" aria-roledescription="architecture"><g class="architecture-groups"><g transform="translate(40,17)" text-anchor="start"><rect class="node-bkg" x="10" y="0" width="80" height="40"></rect><path d="M 1,57 L 2,57 L3,57 "></path></g></g></svg>"#,
            [
                "architecture-beta\nservice renderer(server)[Renderer]",
                "architecture-beta\nservice api(server)[API]",
            ],
        ),
        (
            r#"<svg id="id" width="300" height="180" style="max-width: 300px;" viewBox="13 17 250 180" role="graphics-document document" aria-roledescription="c4"><g><line marker-end="url(#arrow)" x1="12" y1="23" x2="45" y2="56"></line><text x="21" y="31">edge-a</text><text x="34" y="41">edge-b</text></g></svg>"#,
            ["C4Dynamic\nPerson(a, \"A\")", "C4Dynamic\nPerson(b, \"B\")"],
        ),
        (
            r#"<svg id="id" width="300" height="80" style="max-width: 300px;" viewBox="10 20 300 80" role="graphics-document document" aria-roledescription="gitGraph"><g class="commit base commit0"><path d="M10,20 L310,20 L310,100 L10,100 Z"></path></g><g class="commit rust-js commit1"><text x="42" y="53">rust-js</text></g></svg>"#,
            [
                "gitGraph\n commit id: \"base\"\n commit id: \"rust-js\"",
                "gitGraph\n commit id: \"base\"\n commit id: \"rust-js\"\n commit id: \"next\"",
            ],
        ),
        (
            r#"<svg id="id" width="250" height="180" style="max-width: 250px;" viewBox="3 7 250 180" role="graphics-document document" aria-roledescription="treemap"><rect class="treemapSection section1" x="3" y="7" width="250" height="180"></rect><path d="M3,7 L253,187"></path></svg>"#,
            [
                "treemap\n title Runtime cost\n \"Mermaid\" : 45",
                "treemap\n title Updated cost\n \"DOM shim\" : 25",
            ],
        ),
        (
            r#"<svg id="id" width="250" height="180" style="max-width: 250px;" viewBox="3 7 250 180" role="graphics-document document" aria-roledescription="block"><g class="node" id="n1" transform="translate(19,23)"><rect class="basic label-container" x="5" y="6" width="30" height="20"></rect></g><path d="M5,6 L9,10" data-edge="true"></path><text>DB</text><text> wide</text><text>D</text></svg>"#,
            [
                "block-beta\ncolumns 1\nDB\nwide\nD",
                "block-beta\ncolumns 1\nDB\nwide\nD\nA",
            ],
        ),
        (
            r#"<svg id="katana-mermaid-svg-id" width="300" height="220" style="max-width: 300px;" viewBox="13 17 250 220" role="graphics-document document" aria-roledescription="c4"><g><rect x="10" y="157" width="80" height="40"></rect><rect x="110" y="287" width="90" height="45"></rect><rect x="210" y="219" width="70" height="25"></rect><text x="34" y="157">Mobile App</text><text x="144" y="287">Xamarin</text><g transform="translate(42,219)"><text style="font-size: 14px" x="30" y="157">deployment type</text></g></g></svg>"#,
            [
                "C4Deployment\nDeployment_Node(a, \"Device A\")",
                "C4Deployment\nDeployment_Node(b, \"Device B\")",
            ],
        ),
        (
            r##"<svg id="katana-mermaid-svg-id" width="360" height="240" style="max-width: 360px;" viewBox="9 11 360 240" role="graphics-document document" aria-roledescription="c4"><g><rect fill="none" stroke="#AAAAAA" x="20" y="157" width="120" height="80"></rect><rect fill="none" stroke="#AAAAAA" x="180" y="247" width="130" height="95"></rect><text style="font-size: 16px" x="40" y="287">Component A</text><text style="font-size: 12px" x="200" y="319">Component B</text><text style="font-size: 14px" x="40" y="247">Component type</text></g></svg>"##,
            [
                "C4Component\nContainer(a, \"Component A\")",
                "C4Component\nContainer(b, \"Component B\")",
            ],
        ),
    ];

    #[test]
    fn synthetic_geometry_helpers_reject_malformed_inputs() -> Result<(), String> {
        assert_malformed_state_helpers();
        assert_malformed_svg_attributes()?;
        assert_malformed_svg_geometry()
    }

    fn assert_malformed_state_helpers() {
        let malformed_state_cases = [
            (
                "missing state geometry",
                assert_state_attributes_preserved("", "0 0 1 1", "1", "translate(0, 0)"),
            ),
            (
                "missing state anchor",
                assert_state_anchor_preserved("<text/>", Some("start")),
            ),
            (
                "added state anchor",
                assert_state_anchor_preserved(r#"<text text-anchor="start"/>"#, None),
            ),
        ];
        for (name, result) in malformed_state_cases {
            assert!(result.is_err(), "{name} should be rejected");
        }
    }

    fn assert_malformed_svg_attributes() -> Result<(), String> {
        let namespaced = synthetic_root_attributes(r#"<svg xmlns:x="urn:test" x:data="value"/>"#)?;
        assert_eq!(namespaced.get("{urn:test}data"), Some(&"value".to_owned()));

        let malformed_style = {
            let mut attributes = SyntheticRootAttributes::new();
            attributes.insert("style".into(), "width: 1px;".into());
            resize_synthetic_root_attributes(&mut attributes, [0.0, 0.0, 12.0, 13.0])
        };
        assert!(malformed_style.is_err());

        let mut valid_style = SyntheticRootAttributes::new();
        valid_style.insert("style".into(), "max-width: 12px;".into());
        resize_synthetic_root_attributes(&mut valid_style, [0.0, 0.0, 12.0, 13.0])?;
        assert_eq!(
            valid_style.get("style"),
            Some(&"max-width: 12px;".to_owned())
        );

        assert!(expected_synthetic_root_attributes("<svg/>").is_err());
        assert!(synthetic_root_attributes("<g/>").is_err());
        Ok(())
    }

    fn assert_malformed_svg_geometry() -> Result<(), String> {
        assert!(original_synthetic_svg("return { svg: `<svg>${value}</svg>` };").is_err());
        assert!(vary_synthetic_child_geometry("<svg/>".into()).is_err());
        assert!(parse_view_box("0 0 NaN 1").is_err());
        Ok(())
    }

    #[test]
    fn removed_fixture_geometry_corrections_preserve_synthetic_svg_geometry() -> Result<(), String>
    {
        for (raw, sources) in FIXTURE_SENSITIVE_GEOMETRY_CASES {
            let varied = vary_synthetic_geometry(raw)?.replacen(r#"y="157""#, r#"y="163""#, 1);
            assert_fake_svg_geometry_is_source_independent([raw.to_owned(), varied], sources)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "js_runtime_scripts_raw_geometry_tests.rs"]
mod raw_geometry_tests;
