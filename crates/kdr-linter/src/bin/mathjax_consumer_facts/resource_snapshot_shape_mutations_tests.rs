pub(super) const CASES: &[(&str, &str, &str)] = &[
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
];
