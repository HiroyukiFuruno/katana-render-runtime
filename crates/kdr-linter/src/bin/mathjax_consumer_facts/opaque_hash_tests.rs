use super::input_hashes;
use std::collections::BTreeMap;

#[test]
fn reports_missing_opaque_input_from_configured_root() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let sources = BTreeMap::new();

    let error = input_hashes(root.path(), &sources)
        .err()
        .ok_or("missing Cargo.toml must fail")?;
    assert!(error.starts_with("missing-source:Cargo.toml:"), "{error}");
    Ok(())
}
