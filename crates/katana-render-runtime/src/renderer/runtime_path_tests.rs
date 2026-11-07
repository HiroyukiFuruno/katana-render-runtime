use super::RuntimePathResolver;
use crate::markdown::plantuml_renderer::PLANTUML_JAR_VERSION;
use crate::markdown::plantuml_renderer::PlantUmlRendererOps;
use crate::renderer::api::DiagramKind;
use std::path::PathBuf;

#[test]
fn explicit_runtime_path_wins_at_surface_boundary() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from("/tmp/runtime.js");
    let resolved = RuntimePathResolver::resolve(DiagramKind::Mermaid, Some(path.clone()))?;

    assert_eq!(resolved, path);
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
fn drawio_runtime_override_selects_existing_file() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let runtime = temp.path().join("drawio.js");
    std::fs::write(&runtime, b"fixture runtime")?;

    let resolved = RuntimePathResolver::resolve_selected_path(
        DiagramKind::Drawio,
        Some(runtime.as_os_str().to_os_string()),
        None,
    )?;

    assert_eq!(resolved, runtime);
    assert!(resolved.is_file());
    Ok(())
}

#[test]
fn mathjax_without_override_selects_bundled_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let resolved = RuntimePathResolver::resolve_selected_path(DiagramKind::MathJax, None, None)?;

    assert!(resolved.ends_with("generated/mathjax-runtime.min.js"));
    assert!(resolved.is_file());
    Ok(())
}
