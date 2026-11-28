use super::RuntimePathResolver;
use crate::markdown::drawio_renderer::DrawioRendererOps;
use crate::markdown::mathjax_renderer::MathJaxRendererOps;
use crate::markdown::mermaid_renderer::MermaidBinaryOps;
use crate::markdown::plantuml_renderer::PLANTUML_JAR_VERSION;
use crate::markdown::plantuml_renderer::PlantUmlRendererOps;
use crate::renderer::api::{
    DiagramKind, RenderConfig, RenderContext, RenderError, RenderInput, RenderPolicy,
};
use crate::system::ProcessService;
use std::path::PathBuf;

#[test]
fn explicit_runtime_path_wins_at_surface_boundary() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from("/tmp/runtime.js");
    let resolved = RuntimePathResolver::resolve(DiagramKind::Mermaid, Some(path.clone()))?;

    assert_eq!(resolved, path);
    Ok(())
}

#[test]
fn explicit_invalid_utf8_runtime_reports_bundle_read_failure()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("invalid.js");
    std::fs::write(&path, [0xff])?;
    for (kind, source, expected) in [
        (
            DiagramKind::Mermaid,
            "graph TD; A-->B",
            "Failed to read Mermaid.js bundle",
        ),
        (
            DiagramKind::Drawio,
            "<mxGraphModel/>",
            "Failed to read Draw.io JavaScript bundle",
        ),
    ] {
        let (_, renderer) = RuntimePathResolver::resolve_renderer(kind, Some(path.clone()), None)?;
        assert!(matches!(renderer.render(&renderer_input(kind, source)),
            Err(RenderError::Runtime(message)) if message.starts_with(expected)));
    }
    Ok(())
}

#[test]
fn plantuml_cache_dir_can_be_overridden_at_api_boundary() -> Result<(), Box<dyn std::error::Error>>
{
    let kind = DiagramKind::PlantUml;
    let cache_dir = PathBuf::from("/tmp/krr-api-cache");
    let resolved =
        RuntimePathResolver::resolve_with_plantuml_cache_dir(kind, None, Some(cache_dir.clone()))?;

    assert!(resolved.starts_with(cache_dir));
    assert!(resolved.to_string_lossy().contains(PLANTUML_JAR_VERSION));
    assert!(resolved.ends_with("plantuml.jar"));
    Ok(())
}

#[test]
fn plantuml_default_runtime_path_is_resolvable() -> Result<(), Box<dyn std::error::Error>> {
    let resolved = RuntimePathResolver::resolve(DiagramKind::PlantUml, None)?;

    assert!(resolved.ends_with("plantuml.jar"));
    Ok(())
}

#[test]
fn plantuml_renderer_selects_default_and_requested_cache_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let cache = tempfile::tempdir()?;
    let (default, _) = RuntimePathResolver::resolve_renderer(DiagramKind::PlantUml, None, None)?;
    let (selected, _) = RuntimePathResolver::resolve_renderer(
        DiagramKind::PlantUml,
        None,
        Some(cache.path().into()),
    )?;

    assert_eq!(default, PlantUmlRendererOps::default_jar_path());
    assert_eq!(
        selected,
        PlantUmlRendererOps::default_jar_path_for_cache_dir(cache.path())
    );
    Ok(())
}

#[test]
fn javascript_runtime_overrides_select_existing_files() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    for (kind, variable) in javascript_override_cases() {
        let runtime = temp.path().join(format!("{variable}.js"));
        std::fs::write(&runtime, b"fixture runtime")?;
        let resolved = RuntimePathResolver::resolve_selected_path(
            kind,
            Some(runtime.as_os_str().to_os_string()),
            None,
        )?;

        assert_eq!(resolved, runtime);
        assert!(resolved.is_file());
    }
    Ok(())
}

#[test]
fn javascript_without_override_selects_bundled_runtimes() -> Result<(), Box<dyn std::error::Error>>
{
    for (kind, _) in javascript_override_cases() {
        let resolved = RuntimePathResolver::resolve_selected_path(kind, None, None)?;
        let default = default_javascript_path(kind).ok_or("missing default JavaScript path")?;

        assert_eq!(resolved, default);
        assert!(resolved.is_file());
        if kind == DiagramKind::MathJax {
            assert!(resolved.ends_with("generated/mathjax-runtime.min.js"));
        }
    }
    Ok(())
}

fn default_javascript_path(kind: DiagramKind) -> Option<PathBuf> {
    match kind {
        DiagramKind::Mermaid => MermaidBinaryOps::default_install_path(),
        DiagramKind::Drawio => DrawioRendererOps::default_install_path(),
        DiagramKind::MathJax => MathJaxRendererOps::default_install_path(),
        DiagramKind::PlantUml => None,
    }
}

#[test]
fn bundled_renderers_use_packaged_resources_even_when_path_is_missing()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let missing = directory.path().join("missing-runtime.js");
    for (kind, source) in renderer_resource_cases() {
        let renderer = RuntimePathResolver::make_renderer(kind, &missing, true);
        let output = renderer.render(&renderer_input(kind, source))?;
        assert!(output.svg.contains("<svg"), "{kind:?}");
        assert!(output.diagnostics.errors.is_empty(), "{kind:?}");
    }
    Ok(())
}

#[test]
fn external_renderers_report_the_missing_resource_for_each_kind()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let missing = directory.path().join("missing-runtime.js");
    for (kind, source) in renderer_resource_cases() {
        assert_missing_resource_at_resolved_path(kind, source, &missing)?;
    }
    Ok(())
}

#[test]
fn malformed_external_mermaid_bundle_reports_a_read_error() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("mermaid.js");
    std::fs::write(&path, [0xff])?;
    let kind = DiagramKind::Mermaid;
    let (_, renderer) = RuntimePathResolver::resolve_renderer(kind, Some(path), None)?;

    let result = renderer.render(&renderer_input(kind, "graph TD\nA-->B"));

    assert!(matches!(
        result,
        Err(RenderError::Runtime(message)) if message.starts_with("Failed to read Mermaid.js bundle:")
    ));
    Ok(())
}

#[test]
fn empty_javascript_overrides_are_rejected_for_each_runtime()
-> Result<(), Box<dyn std::error::Error>> {
    for (kind, variable) in javascript_override_cases() {
        let result =
            RuntimePathResolver::resolve_selected_path(kind, Some(std::ffi::OsString::new()), None);
        assert!(matches!(result,
            Err(RenderError::RuntimeResolution(message)) if message == format!("{variable} is empty")));
    }
    Ok(())
}

#[test]
fn public_runtime_resolver_validates_environment_in_an_isolated_process()
-> Result<(), Box<dyn std::error::Error>> {
    const CHILD: &str = "KRR_TEST_RUNTIME_PATH_RESOLVER_CHILD";
    const NAME: &str = "renderer::runtime_path::tests::public_runtime_resolver_validates_environment_in_an_isolated_process";
    const VERIFIED: &str = "runtime override contracts verified";
    if std::env::var_os(CHILD).is_some() {
        assert_public_runtime_override_contracts()?;
        println!("{VERIFIED}");
        return Ok(());
    }

    let mut command = ProcessService::create_command(std::env::current_exe()?);
    command
        .args(["--exact", NAME, "--nocapture"])
        .env(CHILD, "1");
    for (_, variable) in javascript_override_cases() {
        command.env(variable, "");
    }
    let output = command.output()?;
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && report.contains(VERIFIED),
        "{report}"
    );
    Ok(())
}

fn assert_public_runtime_override_contracts() -> Result<(), Box<dyn std::error::Error>> {
    for (kind, variable) in javascript_override_cases() {
        let result = RuntimePathResolver::resolve(kind, None);
        assert!(matches!(result,
            Err(RenderError::RuntimeResolution(message)) if message == format!("{variable} is empty")));
        let explicit = PathBuf::from(format!("{variable}.js"));
        assert_eq!(
            RuntimePathResolver::resolve(kind, Some(explicit.clone()))?,
            explicit
        );
    }
    Ok(())
}

fn assert_missing_resource_at_resolved_path(
    kind: DiagramKind,
    source: &str,
    missing: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let (resolved, renderer) =
        RuntimePathResolver::resolve_renderer(kind, Some(missing.to_path_buf()), None)?;
    assert_eq!(resolved.as_path(), missing);
    let result = renderer.render(&renderer_input(kind, source));
    match kind {
        DiagramKind::Drawio | DiagramKind::Mermaid => {
            let expected_name =
                missing_runtime_name(kind).ok_or("unexpected missing-resource case")?;
            assert!(matches!(result,
                Err(RenderError::NotInstalled { kind, install_path, .. })
                    if kind == expected_name && install_path.as_path() == missing));
        }
        DiagramKind::MathJax => {
            let output = result?;
            assert_eq!(output.svg, source);
            assert_eq!(output.diagnostics.errors.len(), 1);
            assert!(
                output.diagnostics.errors[0].contains("MathJax runtime asset is not installed")
            );
        }
        DiagramKind::PlantUml => return Err("unexpected external renderer case".into()),
    }
    Ok(())
}

fn missing_runtime_name(kind: DiagramKind) -> Option<&'static str> {
    match kind {
        DiagramKind::Drawio => Some("Draw.io"),
        DiagramKind::Mermaid => Some("Mermaid"),
        _ => None,
    }
}

fn javascript_override_cases() -> impl Iterator<Item = (DiagramKind, &'static str)> {
    [
        (DiagramKind::Mermaid, "MERMAID_JS"),
        (DiagramKind::Drawio, "DRAWIO_JS"),
        (DiagramKind::MathJax, "MATHJAX_JS"),
    ]
    .into_iter()
}

fn renderer_resource_cases() -> impl Iterator<Item = (DiagramKind, &'static str)> {
    [
        (DiagramKind::Mermaid, "graph TD\nA-->B"),
        (DiagramKind::MathJax, "x + 1"),
        (
            DiagramKind::Drawio,
            r#"<mxGraphModel><root><mxCell id="cell" value="Label" style="text;html=1;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"#,
        ),
    ]
    .into_iter()
}

fn renderer_input(kind: DiagramKind, source: &str) -> RenderInput {
    RenderInput {
        kind,
        source: source.to_owned(),
        config: RenderConfig::default(),
        policy: RenderPolicy::default(),
        context: RenderContext::default(),
    }
}
