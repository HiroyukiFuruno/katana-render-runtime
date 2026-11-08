use super::*;

#[test]
fn cache_guard_rejects_conditionally_removed_identity_check()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let input = RUST_INPUTS[super::super::source::RESOURCE_BYTES_INPUT_INDEX];
    for replacement in [
        "#[cfg(any())]\n        identity.modified?;",
        "#[cfg_attr(all(), cfg(any()))]\n        identity.modified?;",
    ] {
        fixture.replace(input, "identity.modified?;", replacement)?;
        fixture.reject("resource-loader:cache")?;
        fixture.restore(input)?;
    }
    Ok(())
}

#[test]
fn cache_guard_rejects_conditionally_removed_remember_branch()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let input = RUST_INPUTS[super::super::source::RESOURCE_BYTES_INPUT_INDEX];
    let original = "if snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize {";
    for replacement in [
        "#[cfg(any())]\n        if snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize {",
        "#[cfg_attr(all(), cfg(any()))]\n        if snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize {",
    ] {
        fixture.replace(input, original, replacement)?;
        fixture.reject("resource-loader:cache")?;
        fixture.restore(input)?;
    }
    Ok(())
}

#[test]
fn cache_guard_rejects_constructor_closure_argument_attribute()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let input = RUST_INPUTS[super::super::source::RESOURCE_BYTES_INPUT_INDEX];
    inspect_root(fixture.root())?;
    fixture.replace(
        input,
        "|path: &PathBuf, value: &Arc<ResourceSnapshot>|",
        "|#[allow(unused)] path: &PathBuf, value: &Arc<ResourceSnapshot>|",
    )?;
    fixture.reject("resource-loader:cache")?;
    Ok(())
}
