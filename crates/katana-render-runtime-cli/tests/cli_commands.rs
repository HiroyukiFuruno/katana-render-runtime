#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
static NEXT_JUST_PROJECT: AtomicU64 = AtomicU64::new(0);

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn cli_help_uses_krr_root_command() -> TestResult<()> {
    let output = command().arg("--help").output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("Usage: krr <COMMAND>"), "{stdout}");
    assert!(!stdout.contains("Usage: kdr <COMMAND>"), "{stdout}");
    Ok(())
}

#[test]
fn cli_renders_mermaid() -> TestResult<()> {
    let markdown = temp_file("cli-mermaid.md");
    let svg = temp_file("cli-mermaid.svg");
    std::fs::write(&markdown, "```mermaid\ngraph TD; A-->B\n```\n")?;

    let mermaid = command()
        .args(["mermaid", "render", "--input"])
        .arg(&markdown)
        .arg("--output")
        .arg(&svg)
        .env_remove("MERMAID_JS")
        .status()?;
    assert!(mermaid.success());
    assert!(std::fs::read_to_string(&svg)?.contains("<svg"));
    Ok(())
}

#[test]
#[cfg(unix)]
fn cli_delegates_reference_commands_to_just() -> TestResult<()> {
    let just_project = JustProject::new()?;
    let fixtures = just_project.root();

    assert!(reference_status("mermaid", "reference-update", &just_project)?.success());
    assert!(reference_status("mermaid", "compare", &just_project)?.success());
    assert!(reference_status("drawio", "bench", &just_project)?.success());
    assert!(reference_status("plantuml", "bench", &just_project)?.success());
    assert!(!reference_status("drawio", "compare", &just_project)?.success());

    let invocations = std::fs::read_to_string(just_project.invocation_log())?;
    let expected = [
        format!("mermaid-reference\t{}", fixtures.display()),
        format!("mermaid-compare\t{}\t12.5", fixtures.display()),
        format!("drawio-bench\t{}", fixtures.display()),
        format!("plantuml-bench\t{}", fixtures.display()),
        format!("drawio-compare\t{}\t12.5", fixtures.display()),
    ];
    assert_eq!(invocations.lines().collect::<Vec<_>>(), expected);
    Ok(())
}

#[test]
fn cli_plantuml_raw_fallback_logs_warning() -> TestResult<()> {
    let input = temp_file("cli-plantuml.puml");
    std::fs::write(&input, "@startuml\nAlice -> Bob: hello\n@enduml\n")?;

    let output = command()
        .args(["plantuml", "render", "--input"])
        .arg(&input)
        .arg("--runtime")
        .arg(temp_file("missing-plantuml.jar"))
        .output()?;

    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)?.contains("```plantuml"));
    assert!(String::from_utf8(output.stderr)?.contains("plantuml-runtime-unavailable"));
    Ok(())
}

#[test]
fn cli_plantuml_rejects_runtime_and_cache_conflict() -> TestResult<()> {
    let output = command()
        .args(["plantuml", "render", "--input"])
        .arg(temp_file("unread-plantuml.puml"))
        .arg("--runtime")
        .arg(temp_file("runtime.jar"))
        .arg("--cache-dir")
        .arg(temp_file("plantuml-cache"))
        .output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(stderr.contains("--runtime and --cache-dir"), "{stderr}");
    assert!(!stderr.contains("failed to read"), "{stderr}");
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

#[cfg(unix)]
struct JustProject {
    root: PathBuf,
    invocation_log: PathBuf,
}

#[cfg(unix)]
impl JustProject {
    fn new() -> TestResult<Self> {
        let root = std::env::temp_dir().join(format!(
            "krr-cli-just-{}-{}",
            std::process::id(),
            NEXT_JUST_PROJECT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root)?;
        let project = Self {
            invocation_log: root.join("invocations.log"),
            root,
        };
        std::fs::write(
            project.root.join("Justfile"),
            r#"
mermaid-reference fixtures:
    @printf '%s\t%s\n' mermaid-reference "{{fixtures}}" >> "$KRR_TEST_JUST_LOG"

mermaid-compare fixtures min_score:
    @printf '%s\t%s\t%s\n' mermaid-compare "{{fixtures}}" "{{min_score}}" >> "$KRR_TEST_JUST_LOG"

drawio-bench fixtures:
    @printf '%s\t%s\n' drawio-bench "{{fixtures}}" >> "$KRR_TEST_JUST_LOG"

plantuml-bench fixtures:
    @printf '%s\t%s\n' plantuml-bench "{{fixtures}}" >> "$KRR_TEST_JUST_LOG"

drawio-compare fixtures min_score:
    @printf '%s\t%s\t%s\n' drawio-compare "{{fixtures}}" "{{min_score}}" >> "$KRR_TEST_JUST_LOG"
    @exit 7
"#,
        )?;
        Ok(project)
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn invocation_log(&self) -> &Path {
        &self.invocation_log
    }
}

#[cfg(unix)]
impl Drop for JustProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
fn reference_status(
    kind: &str,
    action: &str,
    project: &JustProject,
) -> TestResult<std::process::ExitStatus> {
    let mut cmd = command();
    cmd.current_dir(project.root())
        .env("KRR_TEST_JUST_LOG", project.invocation_log())
        .arg(kind)
        .arg(action)
        .arg("--fixtures")
        .arg(project.root());
    if action == "compare" {
        cmd.args(["--min-score", "12.5"]);
    }
    Ok(cmd.status()?)
}

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_krr"))
}

fn temp_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("krr-{name}-{}", std::process::id()))
}
