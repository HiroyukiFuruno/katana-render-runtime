use super::*;

const RESOURCE_LOADER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "crate::resource_bytes::ResourceBytes::read_text(path)",
        "crate::resource_bytes::ResourceBytes::read_text(other)",
        "resource-loader:external-read",
    ),
    (
        "let snapshot = ResourceBytes::snapshot(path)?;",
        "let snapshot = ResourceBytes::snapshot(other)?;",
        "resource-loader:source",
    ),
    (
        "diagram_runtime/generated/mathjax-runtime.min.js",
        "other.js",
        "resource-loader:mathjax-source",
    ),
    (
        "None => return Self::read_file(path).map(Cow::Owned)",
        "None => return Self::read_file(other).map(Cow::Owned)",
        "resource-loader:external-role",
    ),
];

#[test]
fn mathjax_loader_rejects_changed_source_and_digest_binding()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    fixture.reject_unique_mutations(RUST_INPUTS[9], RESOURCE_LOADER_MUTATIONS)
}

const RESOURCE_BYTES_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "#[derive(Clone, PartialEq, Eq)]",
        "#[derive(Clone)]",
        "resource-loader:metadata",
    ),
    (
        "let file = File::open(path)?;",
        "let file = File::open(other)?;",
        "resource-loader:snapshot",
    ),
    (
        "let mut file = File::open(path)?;",
        "let mut file = File::open(other)?;",
        "resource-loader:bytes-read",
    ),
    (
        "let bytes = Self::read_contents(&mut file)?;",
        "let bytes = Self::read_contents(&mut other)?;",
        "resource-loader:bytes-read",
    ),
    (
        "        Self::read_contents(&mut file)\n",
        "        Self::read_contents(&mut other)\n",
        "resource-loader:bytes-read",
    ),
    (
        "file.read_to_end(&mut bytes)?",
        "other.read_to_end(&mut bytes)?",
        "resource-loader:bytes-read",
    ),
    (
        "Sha256::digest(self.bytes())",
        "Sha256::digest(other.bytes())",
        "resource-loader:digest",
    ),
    (
        "String::from_utf8(bytes)",
        "String::from_utf8(other)",
        "resource-loader:content",
    ),
    (
        "snapshot.identity == *identity",
        "snapshot.identity != *identity",
        "resource-loader:cache",
    ),
    (
        "let snapshot = Self::read_snapshot(file, before)?;",
        "let snapshot = Self::read_snapshot(other, before)?;",
        "resource-loader:snapshot",
    ),
    (
        "let after = Identity::from_metadata(&file.metadata()?);",
        "let after = Identity::from_metadata(&other.metadata()?);",
        "resource-loader:metadata",
    ),
    (
        "before != after || before.len != bytes.len() as u64",
        "before == after || before.len != bytes.len() as u64",
        "resource-loader:changed",
    ),
    (
        "if let Some(snapshot) = Self::cached(cache, path, &before)",
        "if let Some(snapshot) = Self::cached(cache, other, &before)",
        "resource-loader:cache",
    ),
    (
        "Self::remember(cache, path, &snapshot)",
        "Self::remember(cache, other, &snapshot)",
        "resource-loader:snapshot",
    ),
    (
        "cache.insert(path.to_path_buf(), snapshot.clone())",
        "cache.insert(other.to_path_buf(), snapshot.clone())",
        "resource-loader:cache",
    ),
    (
        "ResourceContent::Text(Arc::new(text))",
        "ResourceContent::Text(Arc::new(other))",
        "resource-loader:content",
    ),
    (
        "Ok(Arc::clone(text))",
        "Ok(Arc::clone(other))",
        "resource-loader:accessors",
    ),
];

#[test]
fn mathjax_loader_rejects_changed_byte_acquisition() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    fixture.reject_unique_mutations(
        RUST_INPUTS[super::super::source::RESOURCE_BYTES_INPUT_INDEX],
        RESOURCE_BYTES_MUTATIONS,
    )
}

const BUNDLE_EXECUTION_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),\n            DiagramRuntimeScript::owned(\n                \"render-mathjax.js\",\n                format!(\"katanaRunMathJaxRuntime({request_json});\"),\n            ),",
        "            DiagramRuntimeScript::owned(\n                \"render-mathjax.js\",\n                format!(\"katanaRunMathJaxRuntime({request_json});\"),\n            ),\n            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),",
        "script-order:callee",
    ),
    (
        "katanaRunMathJaxRuntime({request_json});",
        "otherGlobal({request_json});",
        "global-invocation:template",
    ),
    (
        "katanaRunMathJaxRuntime({request_json});",
        "katanaRunMathJaxRuntime();",
        "global-invocation:template",
    ),
    (
        "format!(\"katanaRunMathJaxRuntime({request_json});\")",
        "request_json.to_owned()",
        "global-invocation:format",
    ),
    (
        "format!(\"katanaRunMathJaxRuntime({request_json});\")",
        "concat!(\"katanaRunMathJaxRuntime(\", request_json, \");\")",
        "global-invocation:macro",
    ),
    (
        "format!(\"katanaRunMathJaxRuntime({request_json});\")",
        "format!(\"katanaRunMathJaxRuntime({request_json});\", request_json)",
        "global-invocation:arguments",
    ),
];

const BUNDLE_STRUCTURE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "const MATHJAX_RUNTIME: &str = include_str!(\"../diagram_runtime/generated/mathjax-runtime.min.js\");",
        "const MATHJAX_RUNTIME: &str = \"not-generated\";",
        "include-str-generated:shape",
    ),
    (
        "include_str!(\"../diagram_runtime/generated/mathjax-runtime.min.js\")",
        "include_bytes!(\"../diagram_runtime/generated/mathjax-runtime.min.js\")",
        "include-str-generated:path",
    ),
    (
        "include_str!(\"../diagram_runtime/generated/mathjax-runtime.min.js\")",
        "include_str!(MATHJAX_RUNTIME)",
        "include-str-generated:literal",
    ),
    (
        "MATHJAX_RUNTIME\n    }",
        "format!(\"{MATHJAX_RUNTIME}\")\n    }",
        "include-str-generated:source-function",
    ),
    (
        "        vec![\n            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),",
        "        vec![\n            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),\n            DiagramRuntimeScript::owned(\"extra.js\", runtime_source),",
        "script-order:count",
    ),
    (
        "            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),\n            DiagramRuntimeScript::owned(\n                \"render-mathjax.js\",",
        "            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", String::new()),\n            DiagramRuntimeScript::owned(\n                \"render-mathjax.js\",",
        "script-order:source",
    ),
    (
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source)",
        "DiagramRuntimeScript::other(\"mathjax-runtime.min.js\", runtime_source)",
        "script-order:callee",
    ),
    (
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source)",
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\")",
        "script-order:callee",
    ),
    (
        "vec![\n            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),",
        "scripts![\n            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),",
        "script-order:macro",
    ),
    (
        "vec![\n            DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source),\n            DiagramRuntimeScript::owned(\n                \"render-mathjax.js\",\n                format!(\"katanaRunMathJaxRuntime({request_json});\"),\n            ),\n        ]",
        "vec![DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source); 2]",
        "script-order:array",
    ),
    (
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source)",
        "runtime_source",
        "script-order:entry",
    ),
    (
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source)",
        "(DiagramRuntimeScript::owned)(\"mathjax-runtime.min.js\", runtime_source)",
        "script-order:callee",
    ),
    (
        "DiagramRuntimeScript::borrowed(\"mathjax-runtime.min.js\", runtime_source)",
        "DiagramRuntimeScript::borrowed(runtime_name, runtime_source)",
        "script-order:name",
    ),
    (
        "format!(\"katanaRunMathJaxRuntime({request_json});\")",
        "format!(42)",
        "global-invocation:template",
    ),
];

#[test]
fn mathjax_consumer_facts_ast_linter_rejects_bundle_order_and_invocation_drift()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = repository_root()?;
    let fixture = mathjax_fixture()?;
    assert_eq!(inspect_root(fixture.root())?["status"], "verified");
    reject_bundle_execution_contract_changes(&fixture, &repository)?;
    assert_eq!(inspect_root(fixture.root())?["status"], "verified");
    Ok(())
}

fn reject_bundle_execution_contract_changes(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let scripts = RUST_INPUTS[MATHJAX_JS_SCRIPTS_INDEX];
    let original = repository.join(scripts);
    for &(before, after, expected) in BUNDLE_EXECUTION_MUTATIONS
        .iter()
        .chain(BUNDLE_STRUCTURE_MUTATIONS.iter())
    {
        fixture.replace(scripts, before, after)?;
        fixture.reject(expected)?;
        fixture.restore_from(scripts, &original)?;
    }
    Ok(())
}
