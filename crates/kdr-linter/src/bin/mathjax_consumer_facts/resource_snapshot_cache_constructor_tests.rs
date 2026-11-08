use super::*;

const CONSTRUCTOR_CASES: &[(&str, &str, &str)] = &[
    (
        ".max_capacity(CACHE_BYTES)\n            .weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })",
        ".weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })\n            .max_capacity(0)",
        "resource-loader:cache",
    ),
    (
        ".max_capacity(CACHE_BYTES)\n            .weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })",
        ".weigher(|path, value| 0)\n            .max_capacity(CACHE_BYTES)",
        "resource-loader:cache",
    ),
    (
        ".max_capacity(CACHE_BYTES)\n            .weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })",
        ".weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })\n            .max_capacity(CACHE_BYTES)\n            .max_capacity(CACHE_BYTES)",
        "resource-loader:cache",
    ),
    (
        ".max_capacity(CACHE_BYTES)\n            .weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })",
        ".weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })\n            .max_capacity(CACHE_BYTES)\n            .weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })",
        "resource-loader:cache",
    ),
    (
        ".max_capacity(CACHE_BYTES)\n            .weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })",
        ".weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })\n            .max_capacity(CACHE_BYTES)\n            .other()",
        "resource-loader:cache",
    ),
    (
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "0usize\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "resource-loader:cache",
    ),
    (
        ".max_capacity(CACHE_BYTES)",
        ".max_capacity(0)",
        "resource-loader:cache",
    ),
    (
        ".max_capacity(CACHE_BYTES)",
        ".max_capacity(OTHER_BYTES)",
        "resource-loader:cache",
    ),
    (
        "Cache::builder()",
        "OtherCache::builder()",
        "resource-loader:cache",
    ),
    (
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "0",
        "resource-loader:cache",
    ),
    (
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "other\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "resource-loader:cache",
    ),
    (
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(other.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "resource-loader:cache",
    ),
    (
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(0)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "resource-loader:cache",
    ),
    (
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "value\n                    .bytes()\n                    .len()\n                    \n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "resource-loader:cache",
    ),
    (
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "value\n                    .bytes()\n                    .len()\n                    .wrapping_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "resource-loader:cache",
    ),
    (
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .to_owned()\n                    .unwrap_or(u32::MAX)",
        "resource-loader:cache",
    ),
    (
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)",
        "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(0)",
        "resource-loader:cache",
    ),
    (
        ".weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {",
        ".weigher(|ref path: &PathBuf, value: &Arc<ResourceSnapshot>| {",
        "resource-loader:cache",
    ),
    (
        ".weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {",
        ".weigher(|path @ _: &PathBuf, value: &Arc<ResourceSnapshot>| {",
        "resource-loader:cache",
    ),
    (
        ".weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {",
        ".weigher(|same: &PathBuf, same: &Arc<ResourceSnapshot>| {",
        "resource-loader:cache",
    ),
    (
        ".try_into()\n                    .unwrap_or(u32::MAX)",
        ".try_into()\n                    .unwrap_or(u32::MIN)",
        "resource-loader:cache",
    ),
];

#[test]
fn cache_constructor_rejects_capacity_and_weight_drift() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    fixture.reject_unique_mutations(
        RUST_INPUTS[super::super::source::RESOURCE_BYTES_INPUT_INDEX],
        CONSTRUCTOR_CASES,
    )
}

#[test]
fn cache_constructor_accepts_equivalent_weight_order_and_bindings()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let input = RUST_INPUTS[super::super::source::RESOURCE_BYTES_INPUT_INDEX];
    let before = "value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)";
    let after = "path.as_os_str().len().saturating_add(value.bytes().len()).saturating_add(ENTRY_OVERHEAD).try_into().unwrap_or(u32::MAX)";
    fixture.replace(input, before, after)?;
    inspect_root(fixture.root())?;
    fixture.restore(input)?;
    let original_closure = ".weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })";
    let renamed_closure = ".weigher(|p, v| p.as_os_str().len().saturating_add(v.bytes().len()).saturating_add(ENTRY_OVERHEAD).try_into().unwrap_or(u32::MAX))";
    fixture.replace(input, original_closure, renamed_closure)?;
    inspect_root(fixture.root())?;
    fixture.restore(input)?;
    let alternate = "u32::try_from(ENTRY_OVERHEAD.saturating_add(path.as_os_str().len()).saturating_add(value.bytes().len())).unwrap_or(u32::MAX)";
    fixture.replace(input, before, alternate)?;
    inspect_root(fixture.root())?;
    fixture.restore(input)?;
    Ok(())
}

#[test]
fn cache_constructor_accepts_reversed_builder_settings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let input = RUST_INPUTS[super::super::source::RESOURCE_BYTES_INPUT_INDEX];
    fixture.replace(input, ".max_capacity(CACHE_BYTES)\n            .weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })", ".weigher(|path: &PathBuf, value: &Arc<ResourceSnapshot>| {\n                value\n                    .bytes()\n                    .len()\n                    .saturating_add(path.as_os_str().len())\n                    .saturating_add(ENTRY_OVERHEAD)\n                    .try_into()\n                    .unwrap_or(u32::MAX)\n            })\n            .max_capacity(CACHE_BYTES)")?;
    inspect_root(fixture.root())?;
    Ok(())
}
