use serde_json::{Value, json};

const OBJECT_VERSION: &str = "0.41.0";
const SHA256_HEX_LENGTH: usize = 64;
const OBJECT_FEATURE_COUNT: usize = 2;

pub(crate) struct ParserPin;

fn workspace_object_dependency(workspace: &toml::Value) -> Result<&toml::Value, String> {
    workspace
        .get("workspace")
        .and_then(|value| value.get("dependencies"))
        .and_then(|value| value.get("object"))
        .ok_or_else(|| "parser-pin:workspace-object".into())
}

fn linter_inherits_object(linter: &toml::Value) -> bool {
    linter
        .get("dependencies")
        .and_then(|value| value.get("object"))
        .and_then(|value| value.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true)
}

impl ParserPin {
    pub(crate) fn read() -> Result<Value, String> {
        let workspace = Self::parse_manifest(crate::BUILD_WORKSPACE_MANIFEST, "workspace")?;
        let linter = Self::parse_manifest(crate::BUILD_LINTER_MANIFEST, "linter")?;
        let lock = Self::parse_manifest(crate::BUILD_CARGO_LOCK, "lock")?;
        Self::validate_object_dependency(&workspace, &linter)?;
        let checksum = Self::locked_object_checksum(&lock)?;
        Ok(
            json!({"crate": "object", "version": OBJECT_VERSION, "checksum": checksum,
                  "features": ["read", "std"]}),
        )
    }

    fn parse_manifest(source: &str, label: &str) -> Result<toml::Value, String> {
        toml::from_str(source).map_err(|error| format!("parser-pin:{label}:{error}"))
    }

    fn validate_object_dependency(
        workspace: &toml::Value,
        linter: &toml::Value,
    ) -> Result<(), String> {
        let dependency = workspace_object_dependency(workspace)?
            .as_table()
            .ok_or("parser-pin:workspace-object")?;
        let features = dependency
            .get("features")
            .and_then(toml::Value::as_array)
            .ok_or("parser-pin:features")?;
        if dependency.get("version").and_then(toml::Value::as_str) != Some(OBJECT_VERSION)
            || dependency
                .get("default-features")
                .and_then(toml::Value::as_bool)
                != Some(false)
            || features.len() != OBJECT_FEATURE_COUNT
            || !features
                .iter()
                .any(|feature| feature.as_str() == Some("read"))
            || !features
                .iter()
                .any(|feature| feature.as_str() == Some("std"))
            || !linter_inherits_object(linter)
        {
            return Err("parser-pin:manifest-dependency-drift".into());
        }
        Ok(())
    }

    fn locked_object_checksum(lock: &toml::Value) -> Result<&str, String> {
        let packages = lock
            .get("package")
            .and_then(toml::Value::as_array)
            .ok_or("parser-pin:lock-packages")?;
        let matches = packages
            .iter()
            .filter(|package| package.get("name").and_then(toml::Value::as_str) == Some("object"))
            .filter(|package| {
                package.get("version").and_then(toml::Value::as_str) == Some(OBJECT_VERSION)
            })
            .collect::<Vec<_>>();
        let [package] = matches.as_slice() else {
            return Err("parser-pin:locked-object-missing-or-ambiguous".into());
        };
        let checksum = package
            .get("checksum")
            .and_then(toml::Value::as_str)
            .ok_or("parser-pin:checksum")?;
        if !Self::is_sha256(checksum) {
            return Err("parser-pin:checksum-format".into());
        }
        Ok(checksum)
    }

    fn is_sha256(value: &str) -> bool {
        value.len() == SHA256_HEX_LENGTH
            && value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    }
}

#[cfg(test)]
#[path = "macho_noso_parser_pin_tests.rs"]
mod tests;
