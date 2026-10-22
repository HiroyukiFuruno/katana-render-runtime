use serde_json::Value;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

#[derive(Clone, Copy)]
enum FactsMode {
    Mathjax,
    MermaidClassic,
    NativeWidthHash,
}

const MODES: [FactsMode; 3] = [
    FactsMode::Mathjax,
    FactsMode::MermaidClassic,
    FactsMode::NativeWidthHash,
];

impl FactsMode {
    fn name(self) -> &'static str {
        match self {
            Self::Mathjax => "mathjax",
            Self::MermaidClassic => "mermaid-classic",
            Self::NativeWidthHash => "native-width-hash",
        }
    }

    fn argument(self) -> Option<&'static str> {
        match self {
            Self::Mathjax => None,
            Self::MermaidClassic => Some("--mermaid-classic-scripts"),
            Self::NativeWidthHash => Some("--native-width-hash"),
        }
    }

    fn schema(self) -> &'static str {
        match self {
            Self::Mathjax => "kdr-linter/mathjax-static-facts/v1",
            Self::MermaidClassic => "kdr-linter/mermaid-classic-script-facts/v1",
            Self::NativeWidthHash => "kdr-linter/native-width-hash-facts/v1",
        }
    }
}

fn workspace_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "kdr-linter must remain under the workspace crates directory".to_owned())
}

fn run_facts_mode(mode: FactsMode, root: &Path) -> Result<Value, String> {
    let mut arguments = Vec::new();
    if let Some(argument) = mode.argument() {
        arguments.push(OsString::from(argument));
    }
    arguments.push(root.as_os_str().to_owned());
    let output = run_cli(&arguments)
        .map_err(|error| format!("{} facts mode did not spawn: {error}", mode.name()))?;
    if !output.status.success() {
        return Err(format!(
            "{} facts mode failed: status={:?} stderr={}",
            mode.name(),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("{} facts mode emitted non-UTF-8 JSON: {error}", mode.name()))?;
    serde_json::from_str(&stdout)
        .map_err(|error| format!("{} facts mode emitted invalid JSON: {error}", mode.name()))
}

fn run_cli(arguments: &[OsString]) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_mathjax_consumer_facts"))
        .args(arguments)
        .output()
        .map_err(|error| format!("mathjax_consumer_facts did not spawn: {error}"))
}

fn assert_exit_one(output: &Output, label: &str) -> Result<(), String> {
    if output.status.code() != Some(1) {
        return Err(format!("{label} status was {:?}", output.status.code()));
    }
    Ok(())
}

fn assert_rejected_report(report: &Value) -> Result<(), String> {
    if report["schema"] != "kdr-linter/mathjax-static-facts/v1" || report["status"] != "rejected" {
        return Err(format!("invalid-root report header differed: {report}"));
    }
    let errors = report["errors"]
        .as_array()
        .ok_or_else(|| "invalid-root report errors missing".to_owned())?;
    if errors.is_empty()
        || errors
            .iter()
            .any(|error| error.as_str().map(|value| value.is_empty()).unwrap_or(true))
    {
        return Err(format!("invalid-root report errors differed: {report}"));
    }
    Ok(())
}

fn assert_fact_provenance(mode: FactsMode, report: &Value) -> Result<&[Value], String> {
    if report["schema"] != mode.schema() {
        return Err(format!("{} schema differs", mode.name()));
    }
    if report["status"] != "verified" {
        return Err(format!("{} status is not verified", mode.name()));
    }

    let facts = report["facts"]
        .as_array()
        .ok_or_else(|| format!("{} report must contain an array of facts", mode.name()))?;
    if facts.is_empty() {
        return Err(format!("{} report must contain facts", mode.name()));
    }
    if facts
        .iter()
        .any(|fact| fact["id"].as_str().is_none() || fact["path"].as_str().is_none())
    {
        return Err(format!(
            "{} facts must retain id/path provenance",
            mode.name()
        ));
    }

    Ok(facts)
}

fn assert_input_hashes(mode: FactsMode, report: &Value, root: &Path) -> Result<(), String> {
    let hashes = report["inputSha256"]
        .as_object()
        .ok_or_else(|| format!("{} report must contain inputSha256", mode.name()))?;
    if hashes.is_empty() {
        return Err(format!("{} report must contain input hashes", mode.name()));
    }
    for (path, hash) in hashes {
        let expected = hash
            .as_str()
            .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or_else(|| format!("{} has an invalid input hash for {path}", mode.name()))?;
        let input = Path::new(path);
        let input = if input.is_absolute() {
            input.to_path_buf()
        } else {
            root.join(input)
        };
        let bytes = fs::read(&input)
            .map_err(|error| format!("{} input {path} is unreadable: {error}", mode.name()))?;
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if actual != expected {
            return Err(format!("{} input hash differs for {path}", mode.name()));
        }
    }
    Ok(())
}

fn assert_common_report(mode: FactsMode, report: &Value, root: &Path) -> Result<(), String> {
    assert_fact_provenance(mode, report)?;
    assert_input_hashes(mode, report, root)
}

fn assert_native_width_contract(facts: &[Value], report: &Value) -> Result<(), String> {
    let ids = facts
        .iter()
        .filter_map(|fact| fact["id"].as_str())
        .collect::<Vec<_>>();
    for expected in [
        "hash-fields",
        "svg-id-hash-route",
        "render-cache-hash-route",
    ] {
        if !ids.contains(&expected) {
            return Err(format!("native width facts missing {expected}"));
        }
    }
    if report["nativeExecutionVerified"] != false || report["performanceVerified"] != false {
        return Err("Native width report has unexpected verification flags".to_owned());
    }
    Ok(())
}

fn assert_mode_contract(mode: FactsMode, report: &Value) -> Result<(), String> {
    let facts = report["facts"]
        .as_array()
        .ok_or_else(|| format!("{} report must contain facts", mode.name()))?;
    match mode {
        FactsMode::Mathjax => {
            if report["generatedGlobalVerified"] != false
                || report["overrideContentVerified"] != false
            {
                return Err("MathJax report has unexpected verification flags".to_owned());
            }
        }
        FactsMode::MermaidClassic => {
            if report["sourceProofVerified"] != true
                || report["nativeExecutionVerified"] != false
                || report["requestBytesVerified"] != false
                || report["acceptedConsumerClosure"] != false
            {
                return Err("Mermaid classic report has unexpected verification flags".to_owned());
            }
        }
        FactsMode::NativeWidthHash => assert_native_width_contract(facts, report)?,
    }
    Ok(())
}

#[test]
fn mathjax_consumer_facts_cli_verifies_real_workspace_modes() -> Result<(), String> {
    let root = workspace_root()?;
    for mode in MODES {
        let report = run_facts_mode(mode, &root)?;
        assert_common_report(mode, &report, &root)?;
        assert_mode_contract(mode, &report)?;
    }
    Ok(())
}

#[test]
fn mathjax_consumer_facts_cli_without_arguments_prints_usage_and_exits_one() -> Result<(), String> {
    let output = run_cli(&[])?;
    assert_exit_one(&output, "usage invocation")?;
    if !output.stdout.is_empty() {
        return Err("usage invocation unexpectedly wrote stdout".to_owned());
    }
    let stderr = String::from_utf8(output.stderr)
        .map_err(|error| format!("usage stderr was not UTF-8: {error}"))?;
    if !stderr.contains("usage: mathjax_consumer_facts <repo-root>") {
        return Err(format!("usage stderr did not contain usage text: {stderr}"));
    }
    Ok(())
}

fn held_nonexistent_root() -> Result<(TempDir, PathBuf), String> {
    let directory =
        tempfile::tempdir().map_err(|error| format!("temp root did not create: {error}"))?;
    let root = directory.path().join("missing-repository-root");
    Ok((directory, root))
}

#[test]
fn mathjax_consumer_facts_cli_rejects_a_nonexistent_absolute_root() -> Result<(), String> {
    let (_directory, root) = held_nonexistent_root()?;
    let output = run_cli(&[root.as_os_str().to_owned()])?;
    assert_exit_one(&output, "invalid-root invocation")?;
    let report: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid-root stdout was not JSON: {error}"))?;
    assert_rejected_report(&report)?;
    if output.stderr.is_empty() {
        return Err("invalid-root stderr was empty".to_owned());
    }
    Ok(())
}

#[test]
fn mathjax_consumer_facts_cli_rejects_a_nonexistent_macho_noso_manifest() -> Result<(), String> {
    let (_directory, manifest) = held_nonexistent_root()?;
    let arguments = vec![
        OsString::from("--macho-noso"),
        manifest.as_os_str().to_owned(),
    ];
    let output = run_cli(&arguments)?;
    assert_exit_one(&output, "macho-noso invocation")?;
    if !output.stdout.is_empty() {
        return Err("macho-noso rejection unexpectedly wrote stdout".to_owned());
    }
    if output.stderr.is_empty() {
        return Err("macho-noso stderr was empty".to_owned());
    }
    Ok(())
}
