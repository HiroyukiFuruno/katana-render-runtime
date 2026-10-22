use serde_json::{Value, json};

const OBJECT_VERSION: &str = "0.40.0";
const SHA256_HEX_LENGTH: usize = 64;
const OBJECT_FEATURE_COUNT: usize = 2;

pub(crate) struct ParserPin;

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
        let dependency = workspace["workspace"]["dependencies"]["object"]
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
            || linter["dependencies"]["object"]["workspace"].as_bool() != Some(true)
        {
            return Err("parser-pin:manifest-dependency-drift".into());
        }
        Ok(())
    }

    fn locked_object_checksum(lock: &toml::Value) -> Result<&str, String> {
        let packages = lock["package"]
            .as_array()
            .ok_or("parser-pin:lock-packages")?;
        let matches = packages
            .iter()
            .filter(|package| package["name"].as_str() == Some("object"))
            .filter(|package| package["version"].as_str() == Some(OBJECT_VERSION))
            .collect::<Vec<_>>();
        let [package] = matches.as_slice() else {
            return Err("parser-pin:locked-object-missing-or-ambiguous".into());
        };
        let checksum = package["checksum"].as_str().ok_or("parser-pin:checksum")?;
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
