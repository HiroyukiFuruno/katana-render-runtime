use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn cli_renders_mermaid() -> TestResult<()> {
    let runtime = temp_file("cli-mermaid.js");
    std::fs::write(&runtime, fake_mermaid_bundle())?;
    let markdown = temp_file("cli-mermaid.md");
    let svg = temp_file("cli-mermaid.svg");
    std::fs::write(&markdown, "```mermaid\ngraph TD; A-->B\n```\n")?;

    let mermaid = command()
        .args(["mermaid", "render", "--input"])
        .arg(&markdown)
        .arg("--output")
        .arg(&svg)
        .env("MERMAID_JS", &runtime)
        .status()?;
    assert!(mermaid.success());
    assert!(std::fs::read_to_string(&svg)?.contains("<svg"));
    Ok(())
}

#[test]
fn cli_delegates_reference_commands_to_just() -> TestResult<()> {
    let fixtures = std::env::temp_dir().join(format!("kdr-cli-fixtures-{}", std::process::id()));
    std::fs::create_dir_all(&fixtures)?;
    let success_path = fake_just("success", 0)?;

    assert!(reference_status("mermaid", "reference-update", &fixtures, &success_path)?.success());
    assert!(reference_status("mermaid", "compare", &fixtures, &success_path)?.success());
    assert!(reference_status("drawio", "bench", &fixtures, &success_path)?.success());

    let failure_path = fake_just("failure", 7)?;
    assert!(!reference_status("drawio", "compare", &fixtures, &failure_path)?.success());
    Ok(())
}

#[test]
fn cli_drawio_default_runtime_reports_error_without_stub_svg() -> TestResult<()> {
    let input = temp_file("cli-drawio.drawio");
    let output = temp_file("cli-drawio.svg");
    std::fs::write(&input, "<mxGraphModel><root /></mxGraphModel>")?;

    let status = command()
        .args(["drawio", "render", "--input"])
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .env("DRAWIO_JS", temp_file("missing-drawio.js"))
        .status()?;

    assert!(!status.success());
    assert!(!output.exists());
    Ok(())
}

fn reference_status(
    kind: &str,
    action: &str,
    fixtures: &Path,
    path: &Path,
) -> TestResult<std::process::ExitStatus> {
    let mut cmd = command();
    cmd.arg(kind).arg(action).arg("--fixtures").arg(fixtures);
    if action == "compare" {
        cmd.args(["--min-score", "12.5"]);
    }
    Ok(cmd.env("PATH", path).status()?)
}

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_kdr"))
}

fn fake_just(name: &str, exit_code: i32) -> TestResult<PathBuf> {
    let dir = std::env::temp_dir().join(format!("kdr-fake-just-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let just = dir.join("just");
    std::fs::write(&just, format!("#!/bin/sh\nexit {exit_code}\n"))?;
    let mut permissions = std::fs::metadata(&just)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&just, permissions)?;
    Ok(dir)
}

fn temp_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("kdr-{name}-{}", std::process::id()))
}

fn fake_mermaid_bundle() -> &'static str {
    r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id, source) => ({
    svg: `<svg xmlns="http://www.w3.org/2000/svg" id="${id}" width="20" height="10" viewBox="0 0 20 10"><text>${source}</text></svg>`
  })
};
"#
}
