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
