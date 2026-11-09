use super::*;

#[test]
fn cache_guard_rejects_missing_capacity_or_weigher() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let input = RUST_INPUTS[super::super::source::RESOURCE_BYTES_INPUT_INDEX];
    let weigher = ".weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })";
    let settings = format!(".max_capacity(CACHE_BYTES)\n            {weigher}");
    let settings = settings.as_str();
    let cases = [
        (settings, "", "resource-loader:cache"),
        (
            settings,
            ".max_capacity(CACHE_BYTES)",
            "resource-loader:cache",
        ),
        (settings, weigher, "resource-loader:cache"),
    ];
    fixture.reject_unique_mutations(input, &cases)
}

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

#[test]
fn cache_guard_rejects_attributes_on_insert_and_pending_calls()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let input = RUST_INPUTS[super::super::source::RESOURCE_BYTES_INPUT_INDEX];
    for (call, attributed) in [
        (
            "cache.insert(path.to_path_buf(), snapshot.clone());",
            "#[allow(unused)] cache.insert(path.to_path_buf(), snapshot.clone());",
        ),
        (
            "cache.run_pending_tasks();",
            "#[allow(unused)] cache.run_pending_tasks();",
        ),
    ] {
        fixture.replace(input, call, attributed)?;
        fixture.reject("resource-loader:cache")?;
        fixture.restore(input)?;
    }
    Ok(())
}
