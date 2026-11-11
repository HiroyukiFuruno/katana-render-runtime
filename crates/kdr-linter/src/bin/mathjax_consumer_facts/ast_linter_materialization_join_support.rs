use super::{Fixture, shared_readback};
use std::path::Path;

pub(super) fn reject_install_path_join_mutations(
    fixture: &Fixture,
    original: &Path,
    renderer: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for (before, after) in [
        (".join(\"generated\")", ".join(\"forged-generated\")"),
        (
            ".join(\"katana-render-runtime\")",
            ".join(\"forged-runtime\")",
        ),
    ] {
        fixture.replace(renderer, before, after)?;
        fixture.reject("default-materialize:install-path-join")?;
        fixture.restore_from(renderer, original)?;
    }
    Ok(())
}

pub(super) fn reject_materializer_implementation_changes(
    fixture: &Fixture,
    original: &Path,
    renderer: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    const MATERIALIZE_AT: &str = "    fn materialize_at(path: PathBuf) -> Result<PathBuf, String> {\n        if Self::exists_with_same_bytes(&path)? {\n            return Ok(path);\n        }\n        let Some(parent) = path.parent() else {\n            return Err(\"MathJax generated runtime path has no parent\".to_string());\n        };\n        std::fs::create_dir_all(parent).map_err(runtime_asset_error)?;\n        std::fs::write(&path, MathJaxRuntimeScripts::runtime_source())\n            .map_err(runtime_asset_error)?;\n        Ok(path)\n    }";
    for (replacement, expected) in [
        (
            "    fn materialize_at(_path: PathBuf) -> Result<PathBuf, String> {\n        Ok(PathBuf::from(\"not-the-generated-runtime.js\"))\n    }",
            "default-materializer:materialize-signature",
        ),
        (
            "    fn materialize_at(path: PathBuf) -> Result<PathBuf, String> {\n        Ok(PathBuf::from(\"fake.js\"))\n    }",
            "default-materializer:statement-count",
        ),
    ] {
        fixture.replace(renderer, MATERIALIZE_AT, replacement)?;
        fixture.reject(expected)?;
        fixture.restore_from(renderer, original)?;
    }
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
