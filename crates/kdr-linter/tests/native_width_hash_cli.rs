use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const INPUTS: [&str; 3] = [
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/mod.rs",
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime.rs",
    "crates/katana-render-runtime/src/markdown/mermaid_renderer/render.rs",
];
const MUTATIONS: &[(&str, &str, &str, &str)] = &[
    (
        INPUTS[0],
        "fn hash_mermaid_source_and_preset<H: Hasher>(",
        "fn renamed_hash_mermaid_source_and_preset<H: Hasher>(",
        "missing-function:hash_mermaid_source_and_preset",
    ),
    (
        INPUTS[1],
        "fn svg_id(source: &str, preset: &DiagramColorPreset) -> String",
        "fn renamed_svg_id(source: &str, preset: &DiagramColorPreset) -> String",
        "missing-method:MermaidRenderRequest::svg_id",
    ),
    (
        INPUTS[1],
        "        \"mermaid-svg-id-v1\".hash(&mut hasher);\n",
        "",
        "svg-id-hash-route:statement-count",
    ),
    (
        INPUTS[1],
        "let mut hasher = DefaultHasher::new();",
        "let mut hasher = DefaultHasher::default();",
        "svg-id-hash-route:hasher",
    ),
    (
        INPUTS[1],
        "\"mermaid-svg-id-v1\".hash(&mut hasher);",
        "\"mermaid-svg-id-v1\".hash(&mut DefaultHasher::new());",
        "svg-id-hash-route:domain",
    ),
    (
        INPUTS[1],
        "super::hash_mermaid_source_and_preset(&mut hasher, source, preset);",
        "super::hash_mermaid_source_and_preset(&mut hasher, source.trim(), preset);",
        "svg-id-hash-route:shared-fields",
    ),
    (
        INPUTS[1],
        "format!(\"katana-mermaid-svg-{:016x}\", hasher.finish())",
        "hasher.finish().to_string()",
        "svg-id-hash-route:output",
    ),
    (
        INPUTS[1],
        "format!(\"katana-mermaid-svg-{:016x}\", hasher.finish())",
        "format!(\"katana-mermaid-svg-{:016x}\" hasher.finish())",
        "svg-id-hash-route:output",
    ),
    (
        INPUTS[2],
        ".join(format!(\n                \"{:016x}.{}\",",
        ".join(format!(\n                \"{:016x}.{}\"",
        "render-cache-hash-route:filename",
    ),
    (
        INPUTS[2],
        "fn cache_file_path_with_zenuml_key(",
        "fn renamed_cache_file_path_with_zenuml_key(",
        "missing-method:MermaidRenderOps::cache_file_path_with_zenuml_key",
    ),
    (
        INPUTS[2],
        "super::hash_mermaid_source_and_preset(&mut hasher, source, preset);",
        "super::hash_mermaid_source_and_preset(&mut hasher, preset, source);",
        "render-cache-hash-route:shared-fields",
    ),
    (
        INPUTS[0],
        "preset.arrow.hash(hasher);",
        "preset.arrow.hash(hasher);\n    let _ = ();",
        "hash-fields:signature-or-count",
    ),
    (
        INPUTS[1],
        "format!(\"katana-mermaid-svg-{:016x}\", hasher.finish())",
        "stringify!(\"katana-mermaid-svg-{:016x}\", hasher.finish())",
        "svg-id-hash-route:output",
    ),
    (
        INPUTS[2],
        "preset.dark_mode.hash(&mut hasher);",
        "preset.dark_mode.hash(&mut hasher);\n        let _ = ();",
        "render-cache-hash-route:statement-count",
    ),
    (
        INPUTS[0],
        "use std::hash::{Hash, Hasher};",
        "use std::hash::Hash;",
        "hash-fields:imports",
    ),
    (
        INPUTS[0],
        "    source: &str,\n    preset: &DiagramColorPreset,",
        "    source: &str,",
        "hash-fields:signature",
    ),
    (
        INPUTS[1],
        "fn svg_id(source: &str, preset: &DiagramColorPreset) -> String",
        "fn svg_id(source: &str, preset: &DiagramColorPreset) -> PathBuf",
        "svg-id-hash-route:signature",
    ),
    (
        INPUTS[1],
        "katana-mermaid-svg-{:016x}",
        "katana-mermaid-svg-{:x}",
        "svg-id-hash-route:output",
    ),
    (
        INPUTS[2],
        ".join(format!(\n                \"{:016x}.{}\",",
        ".join(stringify!(\n                \"{:016x}.{}\",",
        "render-cache-hash-route:filename",
    ),
];

fn copy_inputs(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("missing workspace root")?;
    for input in INPUTS {
        let target = root.join(input);
        fs::create_dir_all(target.parent().ok_or("missing parent")?)?;
        fs::copy(repository.join(input), target)?;
    }
    Ok(())
}

fn execute(root: &Path) -> Result<std::process::Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_mathjax_consumer_facts"))
        .arg("--native-width-hash")
        .arg(root)
        .output()
}

fn rejected_diagnostic(root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = execute(root)?;
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    Ok(String::from_utf8(output.stderr)?.trim().to_owned())
}

#[test]
fn native_width_cli_rejects_non_utf8_source_without_report()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    copy_inputs(fixture.path())?;
    let accepted = execute(fixture.path())?;
    assert!(accepted.status.success());
    fs::write(fixture.path().join(INPUTS[1]), [0xff])?;
    assert_eq!(
        rejected_diagnostic(fixture.path())?,
        format!("invalid-utf8:{}", INPUTS[1])
    );
    Ok(())
}

#[test]
fn native_width_cli_rejects_missing_source_and_invalid_rust_without_report()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    copy_inputs(fixture.path())?;
    assert!(execute(fixture.path())?.status.success());
    let source = fixture.path().join(INPUTS[1]);
    fs::write(&source, "fn incomplete(")?;
    let syntax = rejected_diagnostic(fixture.path())?;
    assert!(
        syntax.starts_with(&format!("parse-source:{}:", INPUTS[1])),
        "{syntax}"
    );

    fs::remove_file(source)?;
    let missing = rejected_diagnostic(fixture.path())?;
    assert!(
        missing.starts_with(&format!("missing-source:{}:", INPUTS[1])),
        "{missing}"
    );
    Ok(())
}

#[test]
fn native_width_cli_rejects_a_nonexistent_root_without_report()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let error = rejected_diagnostic(&fixture.path().join("nonexistent"))?;
    assert!(error.starts_with("invalid-root:"), "{error}");
    Ok(())
}

#[test]
fn native_width_cli_rejects_changed_format_contracts_without_report()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    copy_inputs(fixture.path())?;
    let accepted = execute(fixture.path())?;
    assert!(accepted.status.success());
    let report: serde_json::Value = serde_json::from_slice(&accepted.stdout)?;
    assert_eq!(report["status"], "verified");
    for &(input, before, after, expected) in MUTATIONS {
        reject_mutation(
            fixture.path().join(input),
            fixture.path(),
            before,
            after,
            expected,
        )?;
    }
    Ok(())
}

fn reject_mutation(
    path: PathBuf,
    root: &Path,
    before: &str,
    after: &str,
    expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let original = fs::read_to_string(&path)?;
    assert_eq!(original.matches(before).count(), 1);
    fs::write(&path, original.replacen(before, after, 1))?;
    assert_eq!(rejected_diagnostic(root)?, expected);
    fs::write(path, original)?;
    Ok(())
}
