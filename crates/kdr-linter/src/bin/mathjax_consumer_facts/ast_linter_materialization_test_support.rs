use super::{Fixture, MATHJAX_RENDERER_INDEX, OPAQUE_INPUTS, RUST_INPUTS, shared_readback};
use std::path::Path;

#[path = "materializer_body_mutation_support.rs"]
mod body_mutations;

pub(super) const DEFAULT_MATERIALIZATION_SHAPE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "pub fn resolve_mathjax_js() -> Result<PathBuf, String> {",
        "#[inline]\n    pub fn resolve_mathjax_js() -> Result<PathBuf, String> {",
        "default-materializer:renderer-method-attributes",
    ),
    (
        "pub fn resolve_mathjax_js() -> Result<PathBuf, String> {\n",
        "pub fn resolve_mathjax_js() -> Result<PathBuf, String> {\n        let _ = ();\n",
        "explicit-override:statement-count",
    ),
    (
        "    fn resolve_mathjax_js_with_env(\n        env_value: Option<std::ffi::OsString>,\n        bundled_path: Option<PathBuf>,\n    ) -> Result<PathBuf, String> {\n        if let Some(path) = Self::env_mathjax_js_from(env_value)? {",
        "    fn resolve_mathjax_js_with_env(\n        env_value: Option<std::ffi::OsString>,\n        bundled_path: Option<PathBuf>,\n    ) -> Result<PathBuf, String> {\n        let _shape_guard = ();\n        if let Some(path) = Self::env_mathjax_js_from(env_value)? {",
        "default-materialize:statement-count",
    ),
    (
        "std::env::var_os(\"MATHJAX_JS\")",
        "std::env::var(\"MATHJAX_JS\")",
        "explicit-override:default-call",
    ),
    (
        "Self::resolve_mathjax_js_with_env(\n            std::env::var_os(\"MATHJAX_JS\"),\n            Self::default_install_path(),\n        )",
        "Self::resolve_mathjax_js_with_env",
        "explicit-override:call",
    ),
    (
        "Self::resolve_mathjax_js_with_env(\n            std::env::var_os(\"MATHJAX_JS\"),\n            Self::default_install_path(),\n        )",
        "Self::resolve_mathjax_js_with_env(\n            std::env::var_os(\"MATHJAX_JS\"),\n            None,\n        )",
        "explicit-override:default-call",
    ),
    (
        "if let Some(path) = Self::env_mathjax_js_from(env_value)? {",
        "if let Ok(path) = Self::env_mathjax_js_from(env_value) {",
        "default-materialize:env-branch",
    ),
    (
        "let Some(path) = bundled_path else {",
        "let Some(other) = bundled_path else {",
        "default-materialize:path-binding",
    ),
    (
        "let Some(path) = value else {",
        "let Some(other) = value else {",
        "explicit-override:env-bind",
    ),
    (
        "if path.is_empty() {",
        "if path.len() == 0 {",
        "explicit-override:empty-check",
    ),
    (
        "return Err(\"MATHJAX_JS is empty\".to_string())",
        "return Err(\"different error\".to_string())",
        "explicit-override:path-conversion",
    ),
    (
        "    pub fn default_install_path() -> Option<PathBuf> {\n        Some(\n            std::env::temp_dir()\n                .join(\"katana-render-runtime\")\n                .join(\"generated\")\n                .join(\"mathjax-runtime.min.js\"),\n        )\n    }",
        "    pub fn default_install_path() -> Option<PathBuf> {\n        None\n    }",
        "default-materialize:install-path",
    ),
    (
        "std::env::temp_dir()",
        "std::path::PathBuf::new()",
        "default-materialize:install-path-temp-dir",
    ),
    (
        "std::env::var_os(\"MATHJAX_JS\")",
        "env_value",
        "explicit-override:default-call",
    ),
    (
        "if let Some(path) = Self::env_mathjax_js_from(env_value)? {\n            return Ok(path);\n        }",
        "if Self::env_mathjax_js_from(env_value)?.is_some() {}",
        "default-materialize:env-branch",
    ),
    (
        "if let Some(path) = Self::env_mathjax_js_from(env_value)? {\n            return Ok(path);\n        }",
        "let _path = ();",
        "default-materialize:env-branch",
    ),
    (
        "let Some(path) = bundled_path else {\n            return Err(\"bundled MathJax path is unavailable\".to_string());\n        };",
        "if bundled_path.is_none() {}",
        "default-materialize:path-binding",
    ),
    (
        "let Some(path) = bundled_path else {\n            return Err(\"bundled MathJax path is unavailable\".to_string());\n        };",
        "let path;",
        "default-materialize:path-binding",
    ),
    (
        "let Some(path) = value else {\n            return Ok(None);\n        };",
        "if value.is_none() {}",
        "explicit-override:env-bind",
    ),
    (
        "if path.is_empty() {\n            return Err(\"MATHJAX_JS is empty\".to_string());\n        }",
        "let _check = ();",
        "explicit-override:empty-check",
    ),
    (
        "if path.is_empty() {\n            return Err(\"MATHJAX_JS is empty\".to_string());\n        }",
        "if path.is_empty() {}",
        "explicit-override:path-conversion",
    ),
];

pub(in super::super) fn mathjax_fixture() -> Result<Fixture, Box<dyn std::error::Error>> {
    let input_paths = RUST_INPUTS
        .iter()
        .chain(OPAQUE_INPUTS)
        .copied()
        .collect::<Vec<_>>();
    Fixture::copy_mathjax_inputs(&input_paths)
}

pub(in super::super) fn reject_default_materialization_changes(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let renderer = RUST_INPUTS[MATHJAX_RENDERER_INDEX];
    let original = repository.join(renderer);
    fixture.replace(
            renderer,
            "MathJaxGeneratedRuntimeAsset::materialize_at(path)",
            "if false { MathJaxGeneratedRuntimeAsset::materialize_at(path) } else { Err(\"x\".to_string()) }",
        )?;
    fixture.reject("default-materialize:callee:MathJaxGeneratedRuntimeAsset::materialize_at")?;
    fixture.restore_from(renderer, &original)?;

    fixture.replace(
        renderer,
        ".join(\"mathjax-runtime.min.js\")",
        ".join(\"forged.js\")",
    )?;
    fixture.reject("default-materialize:install-path-join")?;
    fixture.restore_from(renderer, &original)?;
    reject_materializer_implementation_changes(fixture, &original, renderer)?;
    reject_default_materialization_shape_changes(fixture, &original, renderer)?;
    Ok(())
}

pub(super) fn reject_default_materialization_shape_changes(
    fixture: &Fixture,
    original: &Path,
    renderer: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for &(before, after, expected) in DEFAULT_MATERIALIZATION_SHAPE_MUTATIONS {
        fixture.replace(renderer, before, after)?;
        fixture.reject(expected)?;
        fixture.restore_from(renderer, original)?;
    }
    body_mutations::reject(fixture, original, renderer)?;
    Ok(())
}

pub(super) fn reject_materializer_implementation_changes(
    fixture: &Fixture,
    original: &Path,
    renderer: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    fixture.replace(
        renderer,
        "    fn materialize_at(path: PathBuf) -> Result<PathBuf, String> {\n        if Self::exists_with_same_bytes(&path)? {\n            return Ok(path);\n        }\n        let Some(parent) = path.parent() else {\n            return Err(\"MathJax generated runtime path has no parent\".to_string());\n        };\n        std::fs::create_dir_all(parent).map_err(runtime_asset_error)?;\n        std::fs::write(&path, MathJaxRuntimeScripts::runtime_source())\n            .map_err(runtime_asset_error)?;\n        Ok(path)\n    }",
        "    fn materialize_at(_path: PathBuf) -> Result<PathBuf, String> {\n        Ok(PathBuf::from(\"not-the-generated-runtime.js\"))\n    }",
    )?;
    fixture.reject("default-materializer:materialize-signature")?;
    fixture.restore_from(renderer, original)?;
    fixture.replace(
        renderer,
        "    fn materialize_at(path: PathBuf) -> Result<PathBuf, String> {\n        if Self::exists_with_same_bytes(&path)? {\n            return Ok(path);\n        }\n        let Some(parent) = path.parent() else {\n            return Err(\"MathJax generated runtime path has no parent\".to_string());\n        };\n        std::fs::create_dir_all(parent).map_err(runtime_asset_error)?;\n        std::fs::write(&path, MathJaxRuntimeScripts::runtime_source())\n            .map_err(runtime_asset_error)?;\n        Ok(path)\n    }",
        "    fn materialize_at(path: PathBuf) -> Result<PathBuf, String> {\n        Ok(PathBuf::from(\"fake.js\"))\n    }",
    )?;
    fixture.reject("default-materializer:statement-count")?;
    fixture.restore_from(renderer, original)?;
    shared_readback::legacy_readback(fixture, renderer)?;
    fixture.replace(
        renderer,
        "struct MathJaxGeneratedRuntimeAsset;\n\nimpl MathJaxGeneratedRuntimeAsset {\n    fn materialize_at(path: PathBuf) -> Result<PathBuf, String> {\n        if Self::exists_with_same_bytes(&path)? {\n            return Ok(path);\n        }\n        let Some(parent) = path.parent() else {\n            return Err(\"MathJax generated runtime path has no parent\".to_string());\n        };\n        std::fs::create_dir_all(parent).map_err(runtime_asset_error)?;\n        std::fs::write(&path, MathJaxRuntimeScripts::runtime_source())\n            .map_err(runtime_asset_error)?;\n        Ok(path)\n    }\n\n    fn exists_with_same_bytes(path: &Path) -> Result<bool, String> {\n        match std::fs::read(path) {\n            Ok(existing) => Ok(existing == MathJaxRuntimeScripts::runtime_source().as_bytes()),\n            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),\n            Err(error) => Err(runtime_asset_error(error)),\n        }\n    }\n}",
        "#[allow(non_snake_case)]\nmod MathJaxGeneratedRuntimeAsset {\n    use std::path::PathBuf;\n\n    pub(super) fn materialize_at(_path: PathBuf) -> Result<PathBuf, String> {\n        Ok(PathBuf::from(\"fake.js\"))\n    }\n}",
    )?;
    fixture.reject("default-materializer:missing-struct")?;
    fixture.restore_from(renderer, original)?;
    Ok(())
}
