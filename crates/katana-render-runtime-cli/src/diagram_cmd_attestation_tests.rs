use super::super::DiagramCommand;
use super::InputFixture;
use crate::commands::{DiagramAction, DiagramInputFormat, DiagramSourceOptions};
use katana_render_runtime::DiagramKind;
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
