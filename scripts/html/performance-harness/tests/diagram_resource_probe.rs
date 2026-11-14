use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const PROBE: &str = env!("CARGO_BIN_EXE_diagram_resource_probe");
const JUSTFILE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../Justfile"));

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn checksum(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn runtime_asset(kind: &str, version_key: &str, filename: &str) -> Result<PathBuf, Box<dyn Error>> {
    let prefix = format!("{version_key} := \"");
    let version = JUSTFILE
        .lines()
        .find_map(|line| {
            line.strip_prefix(&prefix)
                .and_then(|value| value.strip_suffix('"'))
        })
        .ok_or("missing runtime version in Justfile")?;
    Ok(repository().join(format!(
        "crates/katana-render-runtime/vendor/{kind}/{version}/{filename}"
    )))
}

fn probe(kind: &str, source: &Path, runtime: &Path, count: &str) -> std::io::Result<Output> {
    Command::new(PROBE)
        .arg(kind)
        .arg(source)
        .arg(runtime)
        .arg(count)
        .output()
}

fn assert_real_render(kind: &str, source: &Path, runtime: &Path) -> Result<(), Box<dyn Error>> {
    let output = probe(kind, source, runtime, "2")?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["schema"], "krr-diagram-resource-probe-v1");
    assert_eq!(report["kind"], kind);
    assert_eq!(report["input_sha256"], checksum(&fs::read(source)?));
    assert_eq!(report["runtime_sha256"], checksum(&fs::read(runtime)?));
    assert_measurements(&report)
}

fn assert_measurements(report: &Value) -> Result<(), Box<dyn Error>> {
    assert_eq!(report["warm_count"], 2);
    let initial = report["initial_seconds"]
        .as_f64()
        .ok_or("missing initial time")?;
    assert!(initial.is_finite() && initial >= 0.0);
    let warm = report["warm_seconds"]
        .as_array()
        .ok_or("missing warm times")?;
    assert_eq!(warm.len(), 2);
    for value in warm {
        let seconds = value.as_f64().ok_or("invalid warm time")?;
        assert!(seconds.is_finite() && seconds >= 0.0);
    }
    assert!(report["peak_rss_bytes"].as_u64().ok_or("missing RSS")? > 0);
    assert_eq!(report["peak_rss_scope"], "process-lifetime");
    Ok(())
}

#[test]
fn probe_measures_real_mermaid_and_drawio() -> Result<(), Box<dyn Error>> {
    let root = repository();
    let temp = tempfile::tempdir()?;
    let mermaid = temp.path().join("flow.mmd");
    fs::write(&mermaid, "flowchart LR\n  A[Start] --> B[End]\n")?;
    assert_real_render(
        "mermaid",
        &mermaid,
        &runtime_asset("mermaid", "MERMAID_JS_VERSION", "mermaid.min.js")?,
    )?;
    assert_real_render(
        "drawio",
        &root.join("tests/fixtures/drawio/representative/basic-03-basic-flow.drawio"),
        &runtime_asset("drawio", "DRAWIO_JS_VERSION", "drawio.min.js")?,
    )
}

#[test]
fn probe_rejects_missing_input_or_runtime_without_success_report() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("flow.mmd");
    let missing = temp.path().join("missing");
    fs::write(&source, "flowchart LR\n  A --> B\n")?;
    let runtime = runtime_asset("mermaid", "MERMAID_JS_VERSION", "mermaid.min.js")?;
    for (input, asset) in [(&missing, &runtime), (&source, &missing)] {
        let output = probe("mermaid", input, asset, "1")?;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("NotFound"));
    }
    Ok(())
}

#[test]
fn probe_rejects_invalid_mermaid_without_success_report() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("invalid.mmd");
    fs::write(&source, "not a mermaid diagram\n")?;
    let runtime = runtime_asset("mermaid", "MERMAID_JS_VERSION", "mermaid.min.js")?;
    let output = probe("mermaid", &source, &runtime, "1")?;
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Error:"));
    Ok(())
}
