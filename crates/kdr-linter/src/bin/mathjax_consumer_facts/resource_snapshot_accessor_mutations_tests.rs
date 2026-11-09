use super::{RUST_INPUTS, mathjax_fixture};

const DIGEST_INITIALIZER: &str = "self.digest.get_or_init(|| {\n            Sha256::digest(self.bytes())\n                .iter()\n                .map(|byte| format!(\"{byte:02x}\"))\n                .collect()\n        })";

const ACCESSOR_MUTATIONS: &[(&str, &str, &str)] = &[
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
