use super::super::test_support::Fixture;
use std::path::Path;

const PLANTUML_CACHED_CLOSURE: &str = concat!(
    "|path| {\n",
    "                    PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)\n",
    "                }",
);
const PLANTUML_ARM: &str = concat!(
    "            DiagramKind::PlantUml => Ok(plantuml_cache_dir\n",
    "                .map_or_else(PlantUmlRendererOps::default_jar_path, |path| {\n",
    "                    PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)\n",
    "                })),\n",
);

const PLANTUML_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        PLANTUML_ARM,
        "            DiagramKind::PlantUml => plantuml_cache_dir,\n",
        "runtime-path-resolver:plantuml-arm",
    ),
    (
        "DiagramKind::PlantUml => Ok(plantuml_cache_dir",
        "DiagramKind::PlantUml => Err(plantuml_cache_dir",
        "runtime-path-resolver:plantuml-arm",
    ),
    (
        ".map_or_else(PlantUmlRendererOps::default_jar_path",
        ".map_or_else_fake(PlantUmlRendererOps::default_jar_path",
        "runtime-path-resolver:plantuml-map-or-else",
    ),
    (
        PLANTUML_CACHED_CLOSURE,
        "plantuml_mapper",
        "runtime-path-resolver:plantuml-arm",
    ),
    (
        "PlantUmlRendererOps::default_jar_path, |path|",
        "FakePlantUmlRendererOps::default_jar_path, |path|",
        "runtime-path-resolver:plantuml-default",
    ),
    (
        ".map_or_else(PlantUmlRendererOps::default_jar_path",
        ".map_or(PlantUmlRendererOps::default_jar_path",
        "runtime-path-resolver:plantuml-map-or-else",
    ),
    (
        "PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)",
        "PlantUmlRendererOps::default_jar_path_for_cache_dir(path)",
        "runtime-path-resolver:plantuml-cache",
    ),
    (
        "PlantUmlRendererOps::default_jar_path_for_cache_dir(&path)",
        "PlantUmlRendererOps::default_jar_path_for_cache_dir(&path.join(\"nested\"))",
        "runtime-path-resolver:plantuml-cache",
    ),
];

pub(super) fn reject_mutations(
    fixture: &Fixture,
    resolver: &str,
    original: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    for (before, after, expected) in PLANTUML_MUTATIONS {
        replace_legacy_resolver(fixture, resolver, before, after)?;
        fixture.reject(expected)?;
        fixture.restore_from(resolver, original)?;
    }
    Ok(())
}

pub(super) fn replace_legacy_resolver(
    fixture: &Fixture,
    relative: &str,
    before: &str,
    after: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = fixture.root().join(relative);
    let mut source = std::fs::read_to_string(&path)?;
    let start = source
        .find("pub fn resolve_with_plantuml_cache_dir")
        .ok_or("missing legacy resolver")?;
    let end = start
        + source[start..]
            .find("#[cfg(test)]")
            .ok_or("missing test boundary")?;
    let route = &source[start..end];
    if route.matches(before).count() != 1 {
        return Err("legacy resolver mutation is ambiguous".into());
    }
    let offset = route
        .find(before)
        .ok_or("legacy resolver mutation missing")?;
    source.replace_range(start + offset..start + offset + before.len(), after);
    std::fs::write(path, source)?;
    Ok(())
}

const LOADED_RESOLVER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "let env_value = if runtime_path.is_none() {\n            env_name.and_then(std::env::var_os)\n        } else {\n            None\n        };",
        "let env_value = None;",
        "runtime-path-resolver:loaded-env",
    ),
    (
        "let bundled = runtime_path.is_none() && env_name.is_some() && env_value.is_none();",
        "let bundled = false;",
        "runtime-path-resolver:loaded-origin",
    ),
    (
        "let path = match runtime_path {",
        "let path = match other {",
        "runtime-path-resolver:loaded-path",
    ),
    (
        "match kind {\n            DiagramKind::Mermaid => Some(\"MERMAID_JS\"),\n            DiagramKind::Drawio => Some(\"DRAWIO_JS\"),\n            DiagramKind::MathJax => Some(\"MATHJAX_JS\"),\n            DiagramKind::PlantUml => None,\n        }",
        "None",
        "runtime-path-resolver:loaded-env",
    ),
    (
        "DiagramKind::Drawio => Some(\"DRAWIO_JS\"),",
        "",
        "runtime-path-resolver:loaded-env",
    ),
    (
        ") -> Result<PathBuf, RenderError> {\n        match kind {",
        ") -> Result<PathBuf, RenderError> {\n        match other {",
        "runtime-path-resolver:loaded-path",
    ),
    (
        "MathJaxRendererOps::resolve_mathjax_js_with_env(\n                env_value,\n                MathJaxRendererOps::default_install_path(),\n            )",
        "other",
        "runtime-path-resolver:loaded-path",
    ),
    (
        ".map_err(RenderError::RuntimeResolution)\n    }\n\n    fn make_renderer(",
        ".unwrap_or_default()\n    }\n\n    fn make_renderer(",
        "runtime-path-resolver:loaded-path",
    ),
    (
        "let path = match runtime_path {\n            Some(path) => path,\n            None => Self::resolve_selected_path(kind, env_value, plantuml_cache_dir)?,\n        };",
        "let path = runtime_path;",
        "runtime-path-resolver:loaded-path",
    ),
    (
        "None => Self::resolve_selected_path(kind, env_value, plantuml_cache_dir)?,",
        "None => Self::resolve_selected_path(kind, env_value, plantuml_cache_dir)?,\n            _ => other,",
        "runtime-path-resolver:loaded-path",
    ),
    (
        "Ok((path, renderer))",
        "(path, renderer)",
        "runtime-path-resolver:loaded-result",
    ),
    (
        "let env_name = Self::runtime_env_name(kind);",
        "let env_name = Self::runtime_env_name(other);",
        "runtime-path-resolver:loaded-env",
    ),
    (
        "let env_value = if runtime_path.is_none() {",
        "let env_value = if runtime_path.is_some() {",
        "runtime-path-resolver:loaded-env",
    ),
    (
        "None => Self::resolve_selected_path(kind, env_value, plantuml_cache_dir)?,",
        "None => Self::resolve_selected_path(kind, other, plantuml_cache_dir)?,",
        "runtime-path-resolver:loaded-path",
    ),
    (
        "Self::make_renderer(kind, &path, bundled)",
        "Self::make_renderer(kind, &other, bundled)",
        "runtime-path-resolver:loaded-renderer",
    ),
    (
        "Ok((path, renderer))",
        "Ok((renderer, path))",
        "runtime-path-resolver:loaded-result",
    ),
    (
        "DiagramKind::MathJax => Some(\"MATHJAX_JS\")",
        "DiagramKind::MathJax => Some(\"WRONG_JS\")",
        "runtime-path-resolver:loaded-env",
    ),
    (
        "runtime_path.is_none() && env_name.is_some() && env_value.is_none()",
        "runtime_path.is_none() && env_name.is_some() && env_value.is_some()",
        "runtime-path-resolver:loaded-origin",
    ),
    (
        "MathJaxRenderer::with_bundled_runtime_path(path)",
        "MathJaxRenderer::with_runtime_path(path)",
        "runtime-path-resolver:loaded-renderer",
    ),
];

pub(super) fn reject_loaded_mutations(fixture: &Fixture) -> Result<(), Box<dyn std::error::Error>> {
    fixture.reject_unique_mutations(
        super::RUST_INPUTS[super::MATHJAX_RUNTIME_PATH_RESOLVER_INDEX],
        LOADED_RESOLVER_MUTATIONS,
    )
}

pub(super) fn reject_non_match_dispatch_receiver(
    fixture: &Fixture,
    resolver: &str,
    original: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    replace_legacy_resolver(fixture, resolver, "match kind {", "(match kind {")?;
    replace_legacy_resolver(
        fixture,
        resolver,
        "        }\n        .map_err(RenderError::RuntimeResolution)",
        "        })\n        .map_err(RenderError::RuntimeResolution)",
    )?;
    fixture.reject("runtime-path-resolver:dispatch")?;
    fixture.restore_from(resolver, original)?;
    Ok(())
}
