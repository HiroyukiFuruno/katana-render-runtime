use super::{DiagramCommand, tests::InputFixture};
use crate::commands::{DiagramAction, DiagramInputFormat, DiagramSourceOptions};
use katana_render_runtime::{
    DiagramKind, RenderDiagnostics, RenderOutput, RendererProfile, RuntimeVersion,
};
use std::io::ErrorKind;
use std::path::PathBuf;

#[test]
fn real_mermaid_without_attestation_reports_output_results()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = InputFixture::new("input.mmd", "graph TD; A --> B")?;
    let source = format!("graph TD; A --> B\n%% {}", fixture.root.display());
    std::fs::write(&fixture.input, &source)?;
    run_unattested(&fixture, fixture.output.clone())?;
    assert!(std::fs::read_to_string(&fixture.output)?.contains("<svg"));
    let missing = fixture.root.join("missing");
    assert_output_error(&fixture, missing.join("output.svg"), ErrorKind::NotFound)?;
    assert!(!missing.exists());
    let directory_error = if cfg!(windows) {
        ErrorKind::PermissionDenied
    } else {
        ErrorKind::IsADirectory
    };
    assert_output_error(&fixture, fixture.root.clone(), directory_error)?;
    assert!(fixture.root.is_dir());
    assert_eq!(std::fs::read_to_string(&fixture.input)?, source);
    Ok(())
}

fn run_unattested(fixture: &InputFixture, output: PathBuf) -> anyhow::Result<()> {
    DiagramCommand::new(DiagramKind::Mermaid).run(DiagramAction::Render {
        input: fixture.input.clone(),
        output: Some(output),
        runtime: None,
        theme: None,
        theme_from: None,
        theme_mode: None,
        cache_dir: None,
        source_options: DiagramSourceOptions {
            input_format: DiagramInputFormat::DiagramSource,
            render_attestation: None,
            attempt_token: None,
        },
    })
}

fn assert_output_error(
    fixture: &InputFixture,
    output: PathBuf,
    kind: ErrorKind,
) -> Result<(), Box<dyn std::error::Error>> {
    let error = run_unattested(fixture, output.clone())
        .err()
        .ok_or("invalid output path unexpectedly succeeded")?;
    assert_eq!(
        error.to_string(),
        format!("failed to write {}", output.display())
    );
    assert_eq!(
        error
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
        Some(kind)
    );
    Ok(())
}

#[test]
fn write_render_output_handles_file_and_stdout_outputs() -> Result<(), Box<dyn std::error::Error>> {
    let output = render_output();
    let path = std::env::temp_dir().join(format!("krr-cli-output-{}.svg", std::process::id()));

    DiagramCommand::write_render_output(Some(path.clone()), &output)?;
    DiagramCommand::write_render_output(None, &output)?;

    assert_eq!(std::fs::read_to_string(&path)?, "<svg/>");
    std::fs::remove_file(path)?;
    Ok(())
}

fn render_output() -> RenderOutput {
    RenderOutput {
        svg: "<svg/>".to_string(),
        width: 1.0,
        height: 1.0,
        view_box: "0 0 1 1".to_string(),
        runtime: RuntimeVersion {
            name: "test".to_string(),
            version: "0".to_string(),
            checksum: None,
        },
        profile: RendererProfile {
            id: "test".to_string(),
            description: None,
        },
        diagnostics: RenderDiagnostics {
            warnings: vec!["test warning".to_string()],
            errors: Vec::new(),
        },
        cache_fingerprint: "test".to_string(),
    }
}
