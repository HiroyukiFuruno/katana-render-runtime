use super::{RUST_INPUTS, mathjax_fixture};

#[test]
fn snapshot_cache_rejects_discarded_cache_lookup() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    fixture.reject_unique_mutations(
        RUST_INPUTS[super::super::super::source::RESOURCE_BYTES_INPUT_INDEX],
        &[(
            ".filter(|snapshot| snapshot.identity == *identity)",
            ".filter(|snapshot| snapshot.identity == *identity);",
            "resource-loader:tail",
        )],
    )
}

#[test]
fn snapshot_cache_rejects_caching_without_modified_timestamp()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    fixture.reject_unique_mutations(
        RUST_INPUTS[super::super::super::source::RESOURCE_BYTES_INPUT_INDEX],
        &[(
            "snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize",
            "!snapshot.identity.modified.is_some() && weight <= CACHE_BYTES as usize",
            "resource-loader:cache",
        )],
    )
}

#[test]
fn snapshot_loader_rejects_missing_identity_and_cache_hit_guards()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let input = RUST_INPUTS[super::super::super::source::RESOURCE_BYTES_INPUT_INDEX];
    fixture.reject_unique_mutations(input, &[
        (
            "let before = Identity::from_metadata(&file.metadata()?);",
            "let before = file;",
            "resource-loader:metadata",
        ),
        (
            "if let Some(snapshot) = Self::cached(cache, path, &before) {\n            return Ok(snapshot);\n        }",
            "let snapshot = Self::cached(cache, path, &before);",
            "resource-loader:cache",
        ),
        (
            "before != after || before.len != bytes.len() as u64",
            "before",
            "resource-loader:changed",
        ),
    ])
}

#[test]
fn snapshot_loader_rejects_missing_snapshot_and_cache_methods()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    fixture.reject_unique_mutations(
        RUST_INPUTS[super::super::super::source::RESOURCE_BYTES_INPUT_INDEX],
        &[
            (
                "pub(crate) fn snapshot(path: &Path)",
                "pub(crate) fn renamed_snapshot(path: &Path)",
                "missing-method:ResourceBytes::snapshot",
            ),
            (
                "fn read_snapshot(mut file: File, before: Identity)",
                "fn renamed_read_snapshot(mut file: File, before: Identity)",
                "missing-method:ResourceBytes::read_snapshot",
            ),
            (
                "fn cached(\n",
                "fn renamed_cached(\n",
                "missing-method:ResourceBytes::cached",
            ),
        ],
    )
}

const DIGEST_INITIALIZER: &str = "self.digest.get_or_init(|| {\n            Sha256::digest(self.bytes())\n                .iter()\n                .map(|byte| format!(\"{byte:02x}\"))\n                .collect()\n        })";

const ACCESSOR_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "if before != after || before.len != bytes.len() as u64 {\n            return Err(std::io::Error::other(\"resource changed while reading\"));\n        }",
        "let _changed = before != after || before.len != bytes.len() as u64;",
        "resource-loader:changed",
    ),
    (
        "Ok(Arc::new(ResourceSnapshot {\n            content,\n            digest: OnceLock::new(),\n            identity: after,\n        }))",
        "Some(Arc::new(ResourceSnapshot {\n            content,\n            digest: OnceLock::new(),\n            identity: after,\n        }))",
        "resource-loader:result",
    ),
    (
        DIGEST_INITIALIZER,
        "&self.digest",
        "resource-loader:accessors",
    ),
    (
        "ResourceContent::Text(text) => Ok(text.as_str()),\n            ResourceContent::Binary(_) => Err(Self::invalid_utf8()),",
        "ResourceContent::Text(text) => Ok(text.as_str()),\n            ResourceContent::Binary(_) => Ok(\"\"),",
        "resource-loader:accessors",
    ),
    (
        "Self::snapshot(path)?.text().map(str::to_owned)",
        "Self::snapshot(path)?.text().map(|_| String::new())",
        "resource-loader:bytes-text",
    ),
    (
        "std::io::Error::new(\n            std::io::ErrorKind::InvalidData,\n            \"stream did not contain valid UTF-8\",\n        )",
        "std::io::ErrorKind::InvalidData.into()",
        "resource-loader:bytes-text",
    ),
    (
        DIGEST_INITIALIZER,
        "self.digest.get_or_init(String::new)",
        "resource-loader:accessors",
    ),
    (
        DIGEST_INITIALIZER,
        "self.digest.get_or_init(|| String::new())",
        "resource-loader:accessors",
    ),
    (
        "pub(crate) fn bytes(&self) -> &[u8] {\n        match &self.content {\n            ResourceContent::Text(text) => text.as_bytes(),\n            ResourceContent::Binary(bytes) => bytes,\n        }\n    }",
        "pub(crate) fn bytes(&self) -> &[u8] { &[] }",
        "resource-loader:accessors",
    ),
];

#[test]
fn loader_guard_rejects_short_circuited_snapshot_accessors()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let input = RUST_INPUTS[super::super::super::source::RESOURCE_BYTES_INPUT_INDEX];
    fixture.reject_unique_mutations(input, ACCESSOR_MUTATIONS)
}

#[test]
fn loader_guard_rejects_incomplete_digest_and_snapshot_wrappers()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    let input = RUST_INPUTS[super::super::super::source::RESOURCE_BYTES_INPUT_INDEX];
    let digest = "Sha256::digest(self.bytes())\n                .iter()\n                .map(|byte| format!(\"{byte:02x}\"))\n                .collect()";
    let cases = [
        (digest, "digest", "resource-loader:digest"),
        (digest, "digest.collect()", "resource-loader:digest"),
        (
            digest,
            "digest.map(|byte| byte).collect()",
            "resource-loader:digest",
        ),
        (
            "Ok(Arc::new(ResourceSnapshot {\n            content,\n            digest: OnceLock::new(),\n            identity: after,\n        }))",
            "Ok(snapshot)",
            "resource-loader:result",
        ),
        (
            "Self::snapshot(path)?.text().map(str::to_owned)",
            "snapshot",
            "resource-loader:bytes-text",
        ),
    ];
    fixture.reject_unique_mutations(input, &cases)
}
