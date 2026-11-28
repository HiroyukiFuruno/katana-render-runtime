use super::Fixture;
use std::path::Path;

const MATERIALIZER_BODY_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "Self::exists_with_same_bytes(&path)?",
        "Self::exists_with_same_bytes(&path).is_ok()",
        "default-materializer:existing-byte-guard",
    ),
    (
        "if Self::exists_with_same_bytes(&path)? {\n            return Ok(path);\n        }",
        "if Self::exists_with_same_bytes(&path)? {\n            return Ok(path);\n        } else {\n            return Ok(path);\n        }",
        "default-materializer:existing-byte-guard",
    ),
    (
        "path.parent()",
        "path.parent().map(|parent| parent.join(\"nested\"))",
        "default-materializer:parent-binding",
    ),
    (
        "MathJax generated runtime path has no parent",
        "generated runtime parent is unavailable",
        "default-materializer:parent-binding",
    ),
    (
        "std::fs::create_dir_all(parent)",
        "std::fs::create_dir(parent)",
        "default-materializer:create-parent",
    ),
    (
        "std::fs::create_dir_all(parent).map_err(runtime_asset_error)?",
        "std::fs::create_dir_all(parent).map_err(|_| \"failed\".to_string())?",
        "default-materializer:create-parent",
    ),
    (
        "std::fs::write(&path, MathJaxRuntimeScripts::runtime_source())",
        "std::fs::write(&path, String::new())",
        "default-materializer:write-source",
    ),
    (
        "std::fs::write(&path, MathJaxRuntimeScripts::runtime_source())\n            .map_err(runtime_asset_error)?",
        "std::fs::write(&path, MathJaxRuntimeScripts::runtime_source())\n            .map_err(|_| String::new())?",
        "default-materializer:write-source",
    ),
    (
        "        Ok(path)\n    }\n\n    fn exists_with_same_bytes",
        "        Ok(parent.to_path_buf())\n    }\n\n    fn exists_with_same_bytes",
        "default-materializer:return-path",
    ),
];

pub(super) fn reject(
    fixture: &Fixture,
    original: &Path,
    renderer: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for &(before, after, expected) in MATERIALIZER_BODY_MUTATIONS {
        fixture.replace(renderer, before, after)?;
        fixture.reject(expected)?;
        fixture.restore_from(renderer, original)?;
    }
    Ok(())
}
