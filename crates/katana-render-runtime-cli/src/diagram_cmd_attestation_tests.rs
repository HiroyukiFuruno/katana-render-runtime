use super::super::{DiagramCommand, RenderInputFactory, render_input::DiagramRenderRequest};
use super::InputFixture;
use crate::commands::{DiagramAction, DiagramInputFormat, DiagramSourceOptions};
use katana_render_runtime::{DiagramKind, RenderError, RuntimePathResolver};
use std::{fs, path::PathBuf};

fn attested_mermaid_action(fixture: &InputFixture, receipt_path: PathBuf) -> DiagramAction {
    DiagramAction::Render {
        input: fixture.input.clone(),
        output: Some(fixture.output.clone()),
        runtime: None,
        theme: None,
        theme_from: None,
        theme_mode: None,
        cache_dir: None,
        source_options: DiagramSourceOptions {
            input_format: DiagramInputFormat::DiagramSource,
            render_attestation: Some(receipt_path),
            attempt_token: Some("in-process-success".to_string()),
        },
    }
}

#[test]
fn attestation_render_uses_real_mermaid_runtime_in_process()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = InputFixture::new("input.mmd", "")?;
    /* WHY: 実描画の証跡を検証するため、共有SVGキャッシュと衝突しない入力にする。 */
    let source = format!("graph TD; A --> B\n%% {}", fixture.root.display());
    fs::write(&fixture.input, &source)?;
    let receipt_path = fixture.root.join("attestation.json");

    DiagramCommand::new(DiagramKind::Mermaid)
        .run(attested_mermaid_action(&fixture, receipt_path.clone()))?;
    assert!(fs::read_to_string(&fixture.output)?.contains("<svg"));
    let receipt: serde_json::Value = serde_json::from_slice(&fs::read(&receipt_path)?)?;
    assert_eq!(receipt["status"], "output-written");
    assert_eq!(receipt["actualRenderInput"]["source"], source);
    assert_eq!(receipt["attemptToken"], "in-process-success");
    assert_eq!(receipt["actualExit"], serde_json::Value::Null);
    Ok(())
}

#[test]
fn attested_mermaid_syntax_error_keeps_runtime_error_and_publishes_nothing()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "graph TD\nA-->";
    let fixture = InputFixture::new("invalid.mmd", source)?;
    let receipt_path = fixture.root.join("attestation.json");
    let result = DiagramCommand::new(DiagramKind::Mermaid)
        .run(attested_mermaid_action(&fixture, receipt_path.clone()));
    let Err(error) = result else {
        return Err("invalid Mermaid unexpectedly succeeded".into());
    };
    assert!(matches!(
        error.downcast_ref::<RenderError>(),
        Some(RenderError::Runtime(message)) if !message.is_empty()
    ));
    assert_eq!(fs::read_to_string(&fixture.input)?, source);
    assert!(!fixture.output.exists());
    assert!(!receipt_path.exists());
    Ok(())
}

#[test]
fn attested_default_renderer_uses_the_selected_runtime_file() -> anyhow::Result<()> {
    let fixture =
        InputFixture::new("input.mmd", "").map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let request = attested_request(&fixture, None);
    let (path, renderer) = DiagramCommand::new(DiagramKind::Mermaid).prepare_runtime(&request)?;
    let (_, file_renderer) =
        RuntimePathResolver::resolve_renderer(DiagramKind::Mermaid, Some(path), None)?;
    let input = RenderInputFactory::create(
        DiagramKind::Mermaid,
        "graph TD; A-->B".into(),
        serde_json::Value::Null,
    );
    assert_eq!(
        renderer.render(&input)?.cache_fingerprint,
        file_renderer.render(&input)?.cache_fingerprint
    );
    Ok(())
}

fn attested_request(fixture: &InputFixture, runtime: Option<PathBuf>) -> DiagramRenderRequest {
    DiagramRenderRequest {
        input_path: fixture.input.clone(),
        output_path: Some(fixture.output.clone()),
        runtime,
        theme: None,
        theme_from: None,
        theme_mode: None,
        cache_dir: None,
        source_options: DiagramSourceOptions {
            input_format: DiagramInputFormat::DiagramSource,
            render_attestation: Some(fixture.root.join("attestation.json")),
            attempt_token: Some("same-file".to_string()),
        },
    }
}

#[test]
fn replaced_selected_runtime_cannot_produce_a_success_receipt() -> anyhow::Result<()> {
    let fixture = InputFixture::new("input.mmd", "graph TD; A-->B")
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let runtime = fixture.root.join("runtime.js");
    fs::write(&runtime, fake_bundle("before"))?;
    let request = attested_request(&fixture, Some(runtime.clone()));
    let (path, renderer) = DiagramCommand::new(DiagramKind::Mermaid).prepare_runtime(&request)?;
    let source = fs::read_to_string(&fixture.input)?;
    let attestation = DiagramCommand::attestation(&request, &path, &source)?
        .ok_or_else(|| anyhow::anyhow!("missing attestation"))?;
    let input = RenderInputFactory::create(DiagramKind::Mermaid, source, serde_json::Value::Null);
    attestation.capture_input(&input)?;
    let replacement = fixture.root.join("replacement.js");
    fs::write(&replacement, fake_bundle("after"))?;
    #[cfg(windows)]
    fs::remove_file(&runtime)?;
    fs::rename(&replacement, &runtime)?;
    let output = renderer.render(&input)?;
    assert!(output.svg.contains("after"));
    assert!(attestation.complete(&input, &output).is_err());
    assert!(!fixture.root.join("attestation.json").exists());
    Ok(())
}

fn fake_bundle(label: &str) -> String {
    format!(
        r#"globalThis.mermaid = {{ initialize() {{}}, render: async (id) => ({{svg: `<svg xmlns="http://www.w3.org/2000/svg" id="${{id}}" width="20" height="10" viewBox="0 0 20 10"><text>{label}</text></svg>`}}) }};"#
    )
}

#[cfg(unix)]
#[test]
fn explicit_runtime_symlink_is_not_canonicalized_before_attestation() -> anyhow::Result<()> {
    let fixture = InputFixture::new("input.mmd", "graph TD; A-->B")
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let runtime = fixture.root.join("runtime.js");
    let alias = fixture.root.join("alias.js");
    fs::write(&runtime, fake_bundle("runtime"))?;
    std::os::unix::fs::symlink(&runtime, &alias)?;
    let request = attested_request(&fixture, Some(alias.clone()));
    let (path, _) = DiagramCommand::new(DiagramKind::Mermaid).prepare_runtime(&request)?;
    assert_eq!(path, alias);
    assert!(DiagramCommand::attestation(&request, &path, "graph TD; A-->B").is_err());
    Ok(())
}
