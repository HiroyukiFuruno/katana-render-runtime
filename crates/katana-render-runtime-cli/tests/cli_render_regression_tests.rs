use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_FIXTURE_ID: AtomicUsize = AtomicUsize::new(0);

struct TempFixture {
    root: PathBuf,
}

impl TempFixture {
    fn new() -> std::io::Result<Self> {
        let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("krr-cli-render-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for TempFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn write_mermaid_input(root: &TempFixture) -> std::io::Result<PathBuf> {
    let input = root.path("diagram.mmd");
    std::fs::write(&input, "graph TD; A-->B")?;
    Ok(input)
}

fn run_mermaid_render(
    input: &Path,
    output: Option<&Path>,
) -> std::io::Result<std::process::Output> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_krr"));
    command
        .env_remove("MERMAID_JS")
        .args(["mermaid", "render", "--input"])
        .arg(input);
    if let Some(output) = output {
        command.arg("--output").arg(output);
    }
    command.output()
}

#[test]
fn bundled_mermaid_runtime_writes_svg_through_public_cli() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = TempFixture::new()?;
    let input = write_mermaid_input(&fixture)?;
    let output = fixture.path("diagram.svg");
    let result = run_mermaid_render(&input, Some(&output))?;

    assert!(result.status.success(), "{:?}", result);
    assert!(std::fs::read_to_string(output)?.contains("<svg"));
    Ok(())
}

#[test]
fn bundled_zenuml_runtime_writes_svg_through_public_cli() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = TempFixture::new()?;
    let input = fixture.path("diagram.zenuml");
    let source = format!(
        "zenuml\ntitle CLI Order Service {}\n@Actor Client\nClient.method()",
        std::process::id()
    );
    std::fs::write(&input, source)?;
    let output = fixture.path("diagram.svg");
    let result = run_mermaid_render(&input, Some(&output))?;

    assert!(result.status.success(), "{:?}", result);
    let svg = std::fs::read_to_string(output)?;
    assert!(svg.contains("<svg"));
    assert!(svg.contains("CLI Order Service"));
    Ok(())
}

#[test]
fn empty_mermaid_runtime_override_is_rejected_by_public_cli()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = TempFixture::new()?;
    let input = write_mermaid_input(&fixture)?;
    let result = Command::new(env!("CARGO_BIN_EXE_krr"))
        .env("MERMAID_JS", "")
        .args(["mermaid", "render", "--input"])
        .arg(input)
        .output()?;

    assert!(!result.status.success());
    assert!(String::from_utf8(result.stderr)?.contains("MERMAID_JS is empty"));
    Ok(())
}

#[test]
fn missing_mermaid_input_is_reported_by_public_cli() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = TempFixture::new()?;
    let result = run_mermaid_render(&fixture.path("missing.mmd"), None)?;

    assert!(!result.status.success());
    assert!(String::from_utf8(result.stderr)?.contains("failed to read"));
    Ok(())
}

fn assert_attestation_contract(receipt_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let receipt: serde_json::Value = serde_json::from_slice(&std::fs::read(receipt_path)?)?;
    assert_attestation_metadata(&receipt)?;
    assert_runtime_binding(&receipt)?;
    assert_attested_input(receipt_path)?;
    Ok(())
}

fn assert_attestation_metadata(
    receipt: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(receipt["schema"], "katana-native-render-attestation-v1");
    assert_eq!(receipt["status"], "output-written");
    assert_eq!(receipt["attemptToken"], "coverage-contract");
    assert_eq!(receipt["actualRenderInput"]["source"], "graph TD; A-->B");
    assert_eq!(receipt["actualExit"], serde_json::Value::Null);
    assert_eq!(receipt["accepted"], false);
    assert_eq!(receipt["requiresExternalExitEvidence"], true);
    Ok(())
}

fn assert_runtime_binding(receipt: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    let runtime_path = receipt["runtime"]["path"]
        .as_str()
        .ok_or("attested runtime path missing")?;
    assert_eq!(
        std::fs::canonicalize(runtime_path)?,
        Path::new(runtime_path)
    );
    assert_eq!(
        receipt["bindingsBefore"]["runtime"],
        receipt["bindingsAfter"]["runtime"]
    );
    Ok(())
}

fn assert_attested_input(receipt_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let artifacts = receipt_path.with_extension("artifacts");
    assert_eq!(
        std::fs::read(artifacts.join("raw-input.bin"))?,
        b"graph TD; A-->B"
    );
    Ok(())
}

#[test]
fn render_attestation_records_successful_public_cli_render()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = TempFixture::new()?;
    let input = std::fs::canonicalize(write_mermaid_input(&fixture)?)?;
    let canonical_root = std::fs::canonicalize(&fixture.root)?;
    let output = canonical_root.join("diagram.svg");
    let receipt_path = canonical_root.join("render-attestation.json");
    render_and_validate_attestation(&input, &output, &receipt_path)?;
    let runtime_path = receipt_path_runtime(&receipt_path)?;
    let runtime_alias = noncanonical_alias(&runtime_path)?;
    assert_runtime_alias_rejected(&input, &canonical_root, &runtime_alias, false)?;
    assert_runtime_alias_rejected(&input, &canonical_root, &runtime_alias, true)?;
    Ok(())
}

fn render_and_validate_attestation(
    input: &Path,
    output: &Path,
    receipt_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let result = Command::new(env!("CARGO_BIN_EXE_krr"))
        .env_remove("MERMAID_JS")
        .args(["mermaid", "render", "--input"])
        .arg(input)
        .arg("--output")
        .arg(output)
        .arg("--render-attestation")
        .arg(receipt_path)
        .args(["--attempt-token", "coverage-contract"])
        .output()?;
    assert!(result.status.success(), "{:?}", result);
    assert!(std::fs::read_to_string(output)?.contains("<svg"));
    assert_attestation_contract(receipt_path)
}

fn noncanonical_alias(path: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let runtime_path = PathBuf::from(path);
    let runtime_parent = runtime_path.parent().ok_or("runtime parent missing")?;
    let alias = runtime_parent
        .join("..")
        .join(
            runtime_parent
                .file_name()
                .ok_or("runtime parent name missing")?,
        )
        .join(runtime_path.file_name().ok_or("runtime filename missing")?);
    assert_ne!(alias, runtime_path);
    assert_eq!(std::fs::canonicalize(&alias)?, runtime_path);
    Ok(alias)
}

fn receipt_path_runtime(receipt_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let receipt: serde_json::Value = serde_json::from_slice(&std::fs::read(receipt_path)?)?;
    receipt["runtime"]["path"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "attested runtime path missing".into())
}

fn assert_runtime_alias_rejected(
    input: &Path,
    root: &Path,
    runtime_alias: &Path,
    environment_override: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let label = if environment_override {
        "environment"
    } else {
        "cli"
    };
    let output_path = root.join(format!("runtime-alias-{label}.svg"));
    let receipt_path = root.join(format!("runtime-alias-{label}.json"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_krr"));
    command
        .env_remove("MERMAID_JS")
        .args(["mermaid", "render", "--input"])
        .arg(input)
        .arg("--output")
        .arg(&output_path)
        .arg("--render-attestation")
        .arg(&receipt_path)
        .args(["--attempt-token", "coverage-contract"]);
    if environment_override {
        command.env("MERMAID_JS", runtime_alias);
    } else {
        command.arg("--runtime").arg(runtime_alias);
    }
    let result = command.output()?;
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("attestation path must be canonical"));
    assert!(!output_path.exists() && !receipt_path.exists());
    Ok(())
}
