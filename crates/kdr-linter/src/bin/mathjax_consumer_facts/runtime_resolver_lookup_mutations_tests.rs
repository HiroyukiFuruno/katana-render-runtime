use super::{Fixture, MATHJAX_RUNTIME_PATH_RESOLVER_INDEX, Path, RUST_INPUTS, plantuml_tests};

pub(super) fn reject_changes(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = RUST_INPUTS[MATHJAX_RUNTIME_PATH_RESOLVER_INDEX];
    for (name, expected) in [
        (
            "resolve_with_plantuml_cache_dir",
            "missing-method:RuntimePathResolver::resolve_with_plantuml_cache_dir",
        ),
        (
            "resolve_renderer",
            "missing-method:RuntimePathResolver::resolve_renderer",
        ),
    ] {
        fixture.replace(
            path,
            &format!("pub fn {name}("),
            &format!("pub fn renamed_{name}("),
        )?;
        fixture.reject(expected)?;
        fixture.restore_from(path, &repository.join(path))?;
    }
    reject_drawio(fixture, repository, path)
}

fn reject_drawio(
    fixture: &Fixture,
    repository: &Path,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    plantuml_tests::replace_legacy_resolver(
        fixture,
        path,
        "DiagramKind::Drawio => DrawioRendererOps::resolve_drawio_js(),",
        "DiagramKind::Drawio => MermaidBinaryOps::resolve_mermaid_js(),",
    )?;
    fixture.reject("runtime-path-resolver:arm:DiagramKind::Drawio")?;
    fixture.restore_from(path, &repository.join(path))?;
    Ok(())
}
