use super::ParserPin;
use serde_json::json;

const VALID_WORKSPACE: &str = r#"
    [workspace.dependencies]
    object = { version = "0.40.0", default-features = false, features = ["read", "std"] }
"#;
const VALID_LINTER: &str = r#"
    [dependencies]
    object = { workspace = true }
"#;
const VALID_CHECKSUM: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn valid_lock() -> Result<toml::Value, String> {
    toml::from_str(&format!(
        "[[package]]\nname = \"object\"\nversion = \"0.40.0\"\nchecksum = \"{VALID_CHECKSUM}\"\n"
    ))
    .map_err(|error| error.to_string())
}

fn parsed(source: &str) -> Result<toml::Value, String> {
    toml::from_str(source).map_err(|error| error.to_string())
}

#[test]
fn read_returns_the_pinned_object_contract() -> Result<(), String> {
    let report = ParserPin::read()?;
    assert_eq!(report["crate"], "object");
    assert_eq!(report["version"], "0.40.0");
    assert_eq!(report["features"], json!(["read", "std"]));
    assert_eq!(
        report["checksum"].as_str().map(str::len),
        Some(super::SHA256_HEX_LENGTH)
    );
    Ok(())
}

#[test]
fn rejects_malformed_manifest_and_dependency_drift() -> Result<(), String> {
    assert!(ParserPin::parse_manifest("[", "workspace").is_err());
    let workspace = parsed(VALID_WORKSPACE)?;
    let linter = parsed(VALID_LINTER)?;
    ParserPin::validate_object_dependency(&workspace, &linter)?;

    let mut drifted = workspace.clone();
    drifted["workspace"]["dependencies"]["object"]["default-features"] = toml::Value::Boolean(true);
    assert!(ParserPin::validate_object_dependency(&drifted, &linter).is_err());
    assert!(
        ParserPin::validate_object_dependency(&workspace, &toml::Value::Table(Default::default()))
            .is_err()
    );
    Ok(())
}

#[test]
fn rejects_missing_and_ambiguous_lock_entries() -> Result<(), String> {
    let valid = valid_lock()?;
    assert_eq!(
        ParserPin::locked_object_checksum(&valid),
        Ok::<&str, String>(VALID_CHECKSUM)
    );
    for source in [
        format!(
            "[[package]]\nname = \"other\"\nversion = \"0.40.0\"\nchecksum = \"{VALID_CHECKSUM}\"\n"
        ),
        format!(
            "[[package]]\nname = \"object\"\nversion = \"0.40.0\"\nchecksum = \"{VALID_CHECKSUM}\"\n[[package]]\nname = \"object\"\nversion = \"0.40.0\"\nchecksum = \"{VALID_CHECKSUM}\"\n"
        ),
    ] {
        let lock = parsed(&source)?;
        assert!(ParserPin::locked_object_checksum(&lock).is_err());
    }
    Ok(())
}

#[test]
fn rejects_missing_package_catalog_and_incomplete_package_entries() -> Result<(), String> {
    for source in [
        "metadata = 1",
        "package = [\"not-a-table\"]",
        "[[package]]\nversion = \"0.40.0\"",
        "[[package]]\nname = \"object\"",
    ] {
        assert!(ParserPin::locked_object_checksum(&parsed(source)?).is_err());
    }
    Ok(())
}

#[test]
fn rejects_missing_and_malformed_checksums() -> Result<(), String> {
    let valid = valid_lock()?;
    let mut missing_checksum = valid.clone();
    missing_checksum["package"][0]
        .as_table_mut()
        .ok_or("package must be a table")?
        .remove("checksum");
    assert!(ParserPin::locked_object_checksum(&missing_checksum).is_err());
    for checksum in [
        "ABC",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeG",
    ] {
        let mut malformed = valid.clone();
        malformed["package"][0]["checksum"] = toml::Value::String(checksum.into());
        assert!(ParserPin::locked_object_checksum(&malformed).is_err());
    }
    Ok(())
}

#[test]
fn sha256_validation_is_lowercase_hex_and_exact_length() {
    assert!(ParserPin::is_sha256(VALID_CHECKSUM));
    assert!(!ParserPin::is_sha256("ABC"));
    assert!(!ParserPin::is_sha256(
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeG"
    ));
}

#[test]
fn rejects_incomplete_object_dependency_fields() -> Result<(), String> {
    let linter = parsed(VALID_LINTER)?;
    for dependency in [
        "object = { default-features = false, features = [\"read\", \"std\"] }",
        "object = { version = \"0.40.0\", default-features = false }",
        "object = { version = \"0.40.0\", default-features = false, features = [\"read\", \"read\"] }",
        "object = { version = \"0.40.0\", default-features = false, features = [\"std\", \"std\"] }",
        "object = \"0.40.0\"",
    ] {
        let workspace = parsed(&format!("[workspace.dependencies]\n{dependency}\n"))?;
        assert!(ParserPin::validate_object_dependency(&workspace, &linter).is_err());
    }
    Ok(())
}
