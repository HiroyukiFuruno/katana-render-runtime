pub(super) const CASES: &[(&str, &str, &str)] = &[
    (
        "Self {\n            len: metadata.len(),",
        "Other {\n            len: metadata.len(),",
        "resource-loader:metadata",
    ),
    (
        "Self {\n            len: metadata.len(),",
        "Self {\n            unexpected: 0,\n            len: metadata.len(),",
        "resource-loader:metadata",
    ),
    (
        "identity.modified?;",
        "identity.modified?;\n        let ignored = ();",
        "resource-loader:cache",
    ),
    (
        "let weight = snapshot",
        "let ignored = ();\n        let weight = snapshot",
        "resource-loader:cache",
    ),
    (
        "Self {\n            len: metadata.len(),\n            modified: metadata.modified().ok(),\n            created: metadata.created().ok(),\n            #[cfg(unix)]\n            unix: (\n                metadata.dev(),\n                metadata.ino(),\n                metadata.ctime(),\n                metadata.ctime_nsec(),\n            ),\n        }",
        "Self::default()",
        "resource-loader:metadata",
    ),
    (
        "metadata.modified().ok()",
        "metadata.created().ok()",
        "resource-loader:metadata",
    ),
    (
        "metadata.ctime_nsec()",
        "metadata.ctime()",
        "resource-loader:metadata",
    ),
    (
        "before.len != bytes.len() as u64",
        "before.len == bytes.len() as u64",
        "resource-loader:changed",
    ),
    (
        "std::io::ErrorKind::InvalidData",
        "std::io::ErrorKind::InvalidInput",
        "resource-loader:bytes-text",
    ),
    (
        ".map(|byte| format!(\"{byte:02x}\"))",
        ".map(|byte| format!(\"{byte:02X}\"))",
        "resource-loader:digest",
    ),
    (
        "digest: OnceLock::new(),",
        "digest: other,",
        "resource-loader:result",
    ),
    ("identity.modified?;", "other;", "resource-loader:cache"),
    (
        "cache\n            .get(path)\n            .filter(|snapshot| snapshot.identity == *identity)",
        "other",
        "resource-loader:cache",
    ),
    (
        "if snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize {\n            cache.insert(path.to_path_buf(), snapshot.clone());\n            cache.run_pending_tasks();\n        }",
        "other;",
        "resource-loader:cache",
    ),
    ("cache.run_pending_tasks();", "", "resource-loader:cache"),
    (
        "let content = match String::from_utf8(bytes) {\n            Ok(text) => ResourceContent::Text(Arc::new(text)),\n            Err(error) => ResourceContent::Binary(error.into_bytes()),\n        };",
        "let content = other;",
        "resource-loader:content",
    ),
    (
        "Ok(Arc::new(ResourceSnapshot {\n            content,\n            digest: OnceLock::new(),\n            identity: after,\n        }))",
        "other",
        "resource-loader:result",
    ),
    (
        "Arc::new(ResourceSnapshot {\n            content,\n            digest: OnceLock::new(),\n            identity: after,\n        })",
        "other",
        "resource-loader:result",
    ),
    (
        "Self::snapshot(path)?.text().map(str::to_owned)",
        "other",
        "resource-loader:bytes-text",
    ),
    (
        "snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize",
        "true",
        "resource-loader:cache",
    ),
    (
        "snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize",
        "snapshot.identity.modified.is_some()",
        "resource-loader:cache",
    ),
    (
        "snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize",
        "weight <= CACHE_BYTES as usize",
        "resource-loader:cache",
    ),
    (
        "snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize",
        "snapshot.identity.modified.is_none() && weight <= CACHE_BYTES as usize",
        "resource-loader:cache",
    ),
    (
        "snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize",
        "snapshot.identity.modified.is_some() && weight > CACHE_BYTES as usize",
        "resource-loader:cache",
    ),
    (
        "CACHE_BYTES as usize",
        "CACHE_BYTES as u64",
        "resource-loader:cache",
    ),
    (
        "snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize",
        "snapshot.identity.modified.is_some() || weight <= CACHE_BYTES as usize",
        "resource-loader:cache",
    ),
    (
        "let weight = snapshot\n            .bytes()\n            .len()\n            .saturating_add(path.as_os_str().len())\n            .saturating_add(ENTRY_OVERHEAD);",
        "let weight = 0;",
        "resource-loader:cache",
    ),
    (
        "let weight = snapshot\n            .bytes()\n            .len()\n            .saturating_add(path.as_os_str().len())\n            .saturating_add(ENTRY_OVERHEAD);",
        "let weight = snapshot.bytes().len();",
        "resource-loader:cache",
    ),
    (
        "let weight = snapshot\n            .bytes()\n            .len()\n            .saturating_add(path.as_os_str().len())\n            .saturating_add(ENTRY_OVERHEAD);",
        "let weight = other\n            .bytes()\n            .len()\n            .saturating_add(path.as_os_str().len())\n            .saturating_add(ENTRY_OVERHEAD);",
        "resource-loader:cache",
    ),
    (
        "let weight = snapshot\n            .bytes()\n            .len()\n            .saturating_add(path.as_os_str().len())\n            .saturating_add(ENTRY_OVERHEAD);",
        "let weight = snapshot\n            .bytes()\n            .len()\n            .saturating_add(other.as_os_str().len())\n            .saturating_add(ENTRY_OVERHEAD);",
        "resource-loader:cache",
    ),
    (
        "let weight = snapshot\n            .bytes()\n            .len()\n            .saturating_add(path.as_os_str().len())\n            .saturating_add(ENTRY_OVERHEAD);",
        "let weight = snapshot\n            .bytes()\n            .len()\n            .saturating_add(path.as_os_str().len())\n            .saturating_add(0);",
        "resource-loader:cache",
    ),
    (
        "cache.run_pending_tasks();",
        "other.run_pending_tasks();",
        "resource-loader:cache",
    ),
    (
        "cache.run_pending_tasks();",
        "run_pending_tasks!(cache);",
        "resource-loader:cache",
    ),
    (
        "cache.run_pending_tasks();",
        "cache.invalidate_all();",
        "resource-loader:cache",
    ),
    (
        "cache.run_pending_tasks();",
        "cache.run_pending_tasks(extra);",
        "resource-loader:cache",
    ),
    (
        "cache.run_pending_tasks();",
        "noop();",
        "resource-loader:cache",
    ),
    (
        "cache.insert(path.to_path_buf(), snapshot.clone());",
        "#[cfg(any())]\ncache.insert(path.to_path_buf(), snapshot.clone());",
        "resource-loader:cache",
    ),
    (
        "cache.run_pending_tasks();",
        "#[cfg(any())]\ncache.run_pending_tasks();",
        "resource-loader:cache",
    ),
    (
        "cache.run_pending_tasks();",
        "#[cfg(any())] (cache.run_pending_tasks());",
        "resource-loader:cache",
    ),
    (
        "cache.run_pending_tasks();",
        "#[cfg_attr(all(), cfg(any()))]\ncache.run_pending_tasks();",
        "resource-loader:cache",
    ),
    (
        "Self::read_snapshot(file, before)",
        "Self::read_snapshot(file, after)",
        "resource-loader:snapshot",
    ),
    (
        "Self::cached(cache, path, &before)",
        "Self::cached(cache, path, &after)",
        "resource-loader:cache",
    ),
    (
        "return Ok(snapshot);",
        "return Ok(other);",
        "resource-loader:cache",
    ),
    (
        "before != after || before.len != bytes.len() as u64",
        "before != after && before.len != bytes.len() as u64",
        "resource-loader:changed",
    ),
    (
        "std::io::Error::other(\"resource changed while reading\")",
        "std::io::Error::other(\"unrelated failure\")",
        "resource-loader:changed",
    ),
    (
        "CACHE.get_or_init(Self::cache)",
        "CACHE.get_or_init(other)",
        "resource-loader:cache",
    ),
    (
        "identity: after,",
        "identity: before,",
        "resource-loader:result",
    ),
    (
        "ResourceContent::Text(text) => text.as_bytes(),",
        "ResourceContent::Text(text) => text.as_str(),",
        "resource-loader:accessors",
    ),
    (
        "Ok(text) => ResourceContent::Text(Arc::new(text)),",
        "Ok(other) => ResourceContent::Text(Arc::new(text)),",
        "resource-loader:content",
    ),
    (
        "file.read_to_end(&mut bytes)?;",
        "file.read_to_string(&mut bytes)?;",
        "resource-loader:bytes-read",
    ),
    (
        "self.digest.get_or_init(|| {",
        "self.other.get_or_init(|| {",
        "resource-loader:accessors",
    ),
    (
        "ResourceContent::Binary(bytes) => bytes,",
        "ResourceContent::Binary(bytes) => &[],",
        "resource-loader:accessors",
    ),
];

#[test]
fn cache_guard_accepts_equivalent_condition_and_weight_order()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = super::mathjax_fixture()?;
    let input = super::RUST_INPUTS[super::super::super::source::RESOURCE_BYTES_INPUT_INDEX];
    let condition = "snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize";
    for replacement in [
        "(snapshot.identity.modified.is_some()) && (weight <= CACHE_BYTES as usize)",
        "weight <= CACHE_BYTES as usize && snapshot.identity.modified.is_some()",
        "snapshot.identity.modified.is_some() && CACHE_BYTES as usize >= weight",
        "!snapshot.identity.modified.is_none() && weight <= (CACHE_BYTES as usize)",
        "(&snapshot.identity.modified).is_some() && weight <= CACHE_BYTES as (usize)",
    ] {
        fixture.replace(input, condition, replacement)?;
        super::inspect_root(fixture.root())?;
        fixture.restore(input)?;
    }
    let weight = "let weight = snapshot\n            .bytes()\n            .len()\n            .saturating_add(path.as_os_str().len())\n            .saturating_add(ENTRY_OVERHEAD);";
    for replacement in [
        "let weight = path.as_os_str().len().saturating_add(snapshot.bytes().len()).saturating_add(ENTRY_OVERHEAD);",
        "let weight: usize = (snapshot.bytes().len().saturating_add(path.as_os_str().len()).saturating_add(ENTRY_OVERHEAD));",
    ] {
        fixture.replace(input, weight, replacement)?;
        super::inspect_root(fixture.root())?;
        fixture.restore(input)?;
    }
    Ok(())
}

#[test]
fn cache_guard_accepts_equivalent_pending_receiver() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = super::mathjax_fixture()?;
    let input = super::RUST_INPUTS[super::super::super::source::RESOURCE_BYTES_INPUT_INDEX];
    for replacement in [
        "(&cache).run_pending_tasks();",
        "(cache.run_pending_tasks());",
    ] {
        fixture.replace(input, "cache.run_pending_tasks();", replacement)?;
        super::inspect_root(fixture.root())?;
        fixture.restore(input)?;
    }
    Ok(())
}
