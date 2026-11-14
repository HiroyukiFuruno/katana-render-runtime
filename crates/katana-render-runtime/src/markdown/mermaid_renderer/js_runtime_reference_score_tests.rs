use super::{MermaidJsRuntimeOps, geometry_contract};
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::runtime_assets::RuntimeAsset;

pub(super) const GIT_GRAPH_SOURCE: &str = "gitGraph
    commit id: \"base\"
    branch feature
    checkout feature
    commit id: \"rust-js\"
    checkout main
    merge feature";

const C4_DYNAMIC_SOURCE: &str = "C4Dynamic
    title Dynamic diagram for API Application
    Container(spa, \"Single Page Application\", \"javascript and react\")
    Container(api, \"API Application\", \"Java and Spring Boot\")
    Rel(spa, api, \"Uses\", \"JSON/HTTPS\")";

const C4_DYNAMIC_SOURCE_JA: &str = "C4Dynamic
    title APIアプリケーションのダイナミック図
    Container(spa, \"シングルページアプリケーション\", \"JavaScript と React\")
    Container(api, \"APIアプリケーション\", \"Java と Spring Boot\")
    Rel(spa, api, \"利用する\", \"JSON/HTTPS\")";

pub(super) const C4_DEPLOYMENT_SOURCE: &str = "C4Deployment
    title Deployment diagram for Internet Banking System
    Deployment_Node(mob, \"Customer's mobile device\", \"Apple iOS or Android\") {
        Container(mobile, \"Mobile App\", \"Xamarin\")
    }";

pub(super) const ARCHITECTURE_SERVICE_ICONS_SOURCE: &str = "architecture-beta
    group api(cloud)[API]

    service db(database)[Database] in api
    service disk1(disk)[Storage] in api
    service disk2(disk)[Storage] in api
    service server(server)[Server] in api

    db:L -- R:server
    disk1:T -- B:server
    disk2:T -- B:db";

const ARCHITECTURE_SIMPLE_EN_SOURCE: &str = "architecture-beta
    group app(cloud)[KatanA]
    service markdown(server)[Markdown] in app
    service renderer(server)[Renderer] in app
    service svg(database)[SVG cache] in app
    markdown:R -- L:renderer
    renderer:R -- L:svg";

const ARCHITECTURE_SIMPLE_JA_SOURCE: &str = "architecture-beta
    group app(cloud)[KatanA]
    service markdown(server)[Markdown] in app
    service renderer(server)[レンダラー] in app
    service svg(database)[SVGキャッシュ] in app
    markdown:R -- L:renderer
    renderer:R -- L:svg";

const GIT_FONT_OBSERVER: &str = r#"(() => {
      const native = globalThis.__katanaMeasureSvgTextMetrics;
      if (typeof native !== 'function') throw new Error('real SVG metric bridge required');
      const mainFamilies = [];
      globalThis.__katanaMeasureSvgTextMetrics = (text, family, size, weight, italic) => {
        const result = native(text, family, size, weight, italic);
        if (text === 'main') mainFamilies.push(family);
        return result;
      };
      const run = globalThis.katanaRunMermaidRuntime;
      globalThis.katanaRunMermaidRuntime = async (request) => {
        const svg = await run(request);
        if (mainFamilies.length === 0 || mainFamilies.some((family) =>
          family !== 'Noto Sans' && family !== '"Noto Sans"')) {
          throw new Error('Git configured family did not reach real native measurement: ' + JSON.stringify(mainFamilies));
        }
        return svg;
      };
    })();"#;

#[test]
fn runtime_normalizes_git_graph_browser_bounds() -> Result<(), String> {
    let rendered = render_mermaid_svg(GIT_GRAPH_SOURCE);
    let svg = rendered.as_ref().map_err(|error| error.to_owned())?;
    geometry_contract::GeometryContract::git_graph(svg)?;
    Ok(())
}

#[test]
fn runtime_normalizes_c4_dynamic_browser_bounds() -> Result<(), String> {
    let rendered = render_mermaid_svg(C4_DYNAMIC_SOURCE);
    let svg = rendered.as_ref().map_err(|error| error.to_owned())?;
    geometry_contract::GeometryContract::dynamic(svg, false)?;
    Ok(())
}

#[test]
fn runtime_normalizes_c4_dynamic_relation_positions() -> Result<(), String> {
    let rendered = render_mermaid_svg(C4_DYNAMIC_SOURCE);
    let svg = rendered.as_ref().map_err(|error| error.to_owned())?;
    geometry_contract::GeometryContract::dynamic(svg, false)?;
    Ok(())
}

#[test]
fn runtime_normalizes_c4_dynamic_relation_positions_ja_x_offsets() -> Result<(), String> {
    let rendered = render_mermaid_svg(C4_DYNAMIC_SOURCE_JA);
    let svg = rendered.as_ref().map_err(|error| error.to_owned())?;
    geometry_contract::GeometryContract::dynamic(svg, true)?;
    Ok(())
}

#[test]
fn runtime_normalizes_c4_deployment_browser_position_corrections() -> Result<(), String> {
    let rendered = render_mermaid_svg(C4_DEPLOYMENT_SOURCE);
    let svg = rendered.as_ref().map_err(|error| error.to_owned())?;
    geometry_contract::GeometryContract::deployment(svg)?;
    Ok(())
}

#[test]
fn runtime_restores_architecture_service_icons_from_source_types() -> Result<(), String> {
    let rendered = render_mermaid_svg(ARCHITECTURE_SERVICE_ICONS_SOURCE);
    let svg = rendered.as_ref().map_err(|error| error.to_owned())?;
    geometry_contract::GeometryContract::architecture(
        svg,
        &[
            ("db", "Database"),
            ("disk1", "Storage"),
            ("disk2", "Storage"),
            ("server", "Server"),
        ],
        &[
            ("db", "server", 'L', 'R'),
            ("disk1", "server", 'T', 'B'),
            ("disk2", "db", 'T', 'B'),
        ],
        ("api", "API"),
    )?;
    geometry_contract::GeometryContract::icons(svg)?;
    assert_svg_contains(&rendered, r#"rx="20" ry="7.14""#);
    assert_svg_contains(&rendered, "l-4.83,13.22");
    Ok(())
}

#[test]
fn runtime_normalizes_architecture_simple_renderer_coordinates_en() -> Result<(), String> {
    let rendered = render_mermaid_svg(ARCHITECTURE_SIMPLE_EN_SOURCE);
    let svg = rendered.as_ref().map_err(|error| error.to_owned())?;
    geometry_contract::GeometryContract::architecture(
        svg,
        &[
            ("markdown", "Markdown"),
            ("renderer", "Renderer"),
            ("svg", "SVG cache"),
        ],
        &[
            ("markdown", "renderer", 'R', 'L'),
            ("renderer", "svg", 'R', 'L'),
        ],
        ("app", "KatanA"),
    )?;
    Ok(())
}

#[test]
fn runtime_normalizes_architecture_simple_renderer_coordinates_ja() -> Result<(), String> {
    let rendered = render_mermaid_svg(ARCHITECTURE_SIMPLE_JA_SOURCE);
    let svg = rendered.as_ref().map_err(|error| error.to_owned())?;
    geometry_contract::GeometryContract::architecture(
        svg,
        &[
            ("markdown", "Markdown"),
            ("renderer", "レンダラー"),
            ("svg", "SVGキャッシュ"),
        ],
        &[
            ("markdown", "renderer", 'R', 'L'),
            ("renderer", "svg", 'R', 'L'),
        ],
        ("app", "KatanA"),
    )?;
    Ok(())
}

pub(super) fn render_mermaid_svg(source: &str) -> Result<String, String> {
    let mermaid = RuntimeAsset::mermaid();
    mermaid
        .materialize_at(mermaid.materialized_path())
        .and_then(|mermaid_js| {
            MermaidJsRuntimeOps::render(source, &mermaid_js, DiagramColorPreset::dark())
        })
}

#[cfg(target_os = "macos")]
#[test]
fn real_timeline_ex_title_matches_independent_official_view_box() -> Result<(), String> {
    let source = mermaid_fixture_source(oracle::TIMELINE_MARKDOWN)?;
    let (actual, observations) = native_font_observer::render_with_native_font_observations(
        &source,
        DiagramColorPreset::dark(),
    )?;
    let result = assert_view_box_matches(
        &actual,
        oracle::TIMELINE_RAW_VIEW_BOX,
        oracle::TIMELINE_RAW_SOURCE_SHA256,
    );
    result.map_err(|error| format!("{error}; native font observations: {observations}"))
}

#[cfg(target_os = "macos")]
#[test]
fn raw_view_box_comparison_rejects_non_finite_components_and_geometry_drift() -> Result<(), String>
{
    let expected = [0.0, 0.0, 10.0, 20.0];
    let source = oracle::TIMELINE_RAW_SOURCE_SHA256;
    let svg = |component: &str| format!(r#"<svg viewBox="0 0 {component} 20"/>"#);
    assert_view_box_matches(&svg("10.01"), expected, source)?;
    for component in ["10.25", "NaN", "inf"] {
        let error = assert_view_box_matches(&svg(component), expected, source)
            .err()
            .ok_or_else(|| format!("invalid viewBox component {component} was accepted"))?;
        assert!(error.contains("component 2 differs") && error.contains(source));
    }
    let mut invalid_reference = expected;
    invalid_reference[2] = f64::NAN;
    assert!(assert_view_box_matches(&svg("10"), invalid_reference, source).is_err());
    Ok(())
}

fn assert_svg_contains(rendered: &Result<String, String>, expected: &str) {
    assert!(
        rendered.as_ref().is_ok_and(|svg| svg.contains(expected)),
        "missing {expected:?} in {rendered:?}"
    );
}

#[test]
fn real_git_graph_resolves_configured_css_font_before_native_measurement() -> Result<(), String> {
    use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};

    let source = format!(
        "%%{{init: {{\"themeVariables\": {{\"fontFamily\": \"Noto Sans\"}}}}}}%%\n{GIT_GRAPH_SOURCE}"
    );
    let asset = RuntimeAsset::mermaid();
    let path = asset.materialize_at(asset.materialized_path())?;
    let bundle = super::read_mermaid_bundle(&path)?;
    let request = super::MermaidRenderRequest::new(&source, DiagramColorPreset::dark());
    let request_json = request.to_json_value().to_string();
    let mut scripts =
        super::MermaidRuntimeScripts::build(&bundle, &request_json, request.diagram_type);
    let observer = GIT_FONT_OBSERVER;
    scripts.insert(
        1,
        DiagramRuntimeScript::borrowed("git-configured-font-boundary", observer),
    );
    let svg = DiagramV8Runtime::render(&scripts)?;
    geometry_contract::GeometryContract::git_graph(&svg)
}

#[test]
fn real_svg_text_css_font_variable_scope_reaches_native_metrics() -> Result<(), String> {
    use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};

    let asset = RuntimeAsset::mermaid();
    let path = asset.materialize_at(asset.materialized_path())?;
    let bundle = super::read_mermaid_bundle(&path)?;
    let request = super::MermaidRenderRequest::new(GIT_GRAPH_SOURCE, DiagramColorPreset::dark());
    let request_json = request.to_json_value().to_string();
    let mut scripts =
        super::MermaidRuntimeScripts::build(&bundle, &request_json, request.diagram_type);
    let fixture = oracle::svg_css_variable_fixture();
    scripts.insert(
        1,
        DiagramRuntimeScript::borrowed("svg-css-font-variable-native-boundary", &fixture),
    );
    let svg = DiagramV8Runtime::render(&scripts)?;
    geometry_contract::GeometryContract::git_graph(&svg)
}

#[path = "js_runtime_reference_score_geometry_boundary_tests.rs"]
mod geometry_boundary_tests;
#[cfg(target_os = "macos")]
#[path = "js_runtime_native_font_observer.rs"]
mod native_font_observer;
#[path = "js_runtime_reference_score_oracle.rs"]
mod oracle;
pub(super) use oracle::SVG_DOM_PLACEMENT_SCALE;

#[cfg(target_os = "macos")]
pub(super) use native_font_observer::render_with_native_font_observations;
#[cfg(target_os = "macos")]
pub(super) use oracle::{assert_view_box_matches, mermaid_fixture_source};
