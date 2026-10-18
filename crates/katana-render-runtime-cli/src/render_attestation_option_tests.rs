use super::{Fixture, unit_output};
use crate::commands::{Cli, Commands, DiagramAction, DiagramInputFormat, DiagramSourceOptions};
use crate::diagram_cmd::{DiagramCommand, RenderInputFactory};
use clap::Parser;
use katana_render_runtime::DiagramKind;
use std::fs;
use std::path::PathBuf;

#[test]
fn attestation_cli_flags_require_output_and_attempt_token() {
    let args = [
        "krr",
        "mermaid",
        "render",
        "--input",
        "in.mmd",
        "--render-attestation",
        "out.json",
    ];
    assert!(Cli::try_parse_from(args).is_err());
    assert!(
        Cli::try_parse_from([
            "krr",
            "mermaid",
            "render",
            "--input",
            "in.mmd",
            "--attempt-token",
            "token"
        ])
        .is_err()
    );
}

fn render_options(cli: Cli) -> anyhow::Result<DiagramSourceOptions> {
    let Commands::Mermaid {
        action: DiagramAction::Render { source_options, .. },
    } = cli.command
    else {
        anyhow::bail!("wrong parsed render kind")
    };
    Ok(source_options)
}

#[test]
fn parses_raw_attestation_flags() -> anyhow::Result<()> {
    let options = render_options(Cli::try_parse_from([
        "krr",
        "mermaid",
        "render",
        "--input",
        "in.mmd",
        "--output",
        "out.svg",
        "--input-format",
        "diagram-source",
        "--render-attestation",
        "out.json",
        "--attempt-token",
        "abc-123",
    ])?)?;
    assert_eq!(options.input_format, DiagramInputFormat::DiagramSource);
    assert_eq!(options.render_attestation, Some(PathBuf::from("out.json")));
    assert_eq!(options.attempt_token.as_deref(), Some("abc-123"));
    Ok(())
}

#[test]
fn keeps_default_auto_without_attestation() -> anyhow::Result<()> {
    let options = render_options(Cli::try_parse_from([
        "krr", "mermaid", "render", "--input", "in.md",
    ])?)?;
    assert_eq!(options.input_format, DiagramInputFormat::Auto);
    assert!(options.render_attestation.is_none() && options.attempt_token.is_none());
    Ok(())
}

#[test]
fn raw_source_preserves_whitespace_crlf_and_embedded_fences() {
    for source in [
        "\n classDiagram\n\n  class 動物  \n",
        "zenuml\r\nA.method()\r\n",
        "```mermaid\ngraph TD; A\n```\n",
    ] {
        assert_eq!(
            DiagramCommand::prepare_source(
                DiagramKind::Mermaid,
                source.into(),
                DiagramInputFormat::DiagramSource
            )
            .as_bytes(),
            source.as_bytes()
        );
    }
    let markdown = "```mermaid\r\ngraph TD; A\r\n```\r\n";
    assert_eq!(
        DiagramCommand::prepare_source(
            DiagramKind::Mermaid,
            markdown.into(),
            DiagramInputFormat::Auto
        ),
        "graph TD; A"
    );
}

#[test]
fn non_mermaid_raw_mode_is_rejected_before_reading_input() -> anyhow::Result<()> {
    let cli = Cli::try_parse_from([
        "krr",
        "drawio",
        "render",
        "--input",
        "missing.drawio",
        "--input-format",
        "diagram-source",
    ])?;
    let Commands::Drawio { action } = cli.command else {
        anyhow::bail!("wrong parsed kind");
    };
    assert!(
        DiagramCommand::new(DiagramKind::Drawio)
            .run(action)
            .is_err_and(|error| error.to_string().contains("only for mermaid"))
    );
    Ok(())
}

#[test]
fn observed_auto_parser_source_diff_is_kept_in_attestation() -> anyhow::Result<()> {
    let markdown = "```zenuml\r\n  A.method()  \r\n```\r\n";
    let fixture = Fixture::new(markdown)?;
    let attestation =
        fixture.begin_with_format(markdown, DiagramInputFormat::Auto, "auto-contract")?;
    let source = DiagramCommand::prepare_source(
        DiagramKind::Mermaid,
        markdown.into(),
        DiagramInputFormat::Auto,
    );
    let input = RenderInputFactory::create(DiagramKind::Mermaid, source, serde_json::Value::Null);
    attestation.capture_input(&input)?;
    attestation.complete(&input, &unit_output())?;
    let receipt: serde_json::Value = serde_json::from_slice(&fs::read(&fixture.receipt)?)?;
    assert_eq!(receipt["inputFormat"], "auto");
    assert_eq!(
        receipt["actualRenderInput"]["source"],
        "zenuml\n  A.method()  "
    );
    assert_ne!(
        receipt["input"]["sha256"],
        receipt["actualRenderSourceSha256"]
    );
    Ok(())
}

#[test]
fn attempt_token_without_attestation_is_rejected_before_input_read() {
    let result = DiagramCommand::new(DiagramKind::Mermaid).run(DiagramAction::Render {
        source_options: DiagramSourceOptions {
            attempt_token: Some("coverage-contract".to_string()),
            ..DiagramSourceOptions::default()
        },
        input: PathBuf::from("missing-input-for-attempt-token.mmd"),
        output: Some(PathBuf::from("unused-output.svg")),
        runtime: None,
        theme: None,
        theme_from: None,
        theme_mode: None,
        cache_dir: None,
    });

    assert!(
        matches!(result, Err(error) if error.to_string() == "--attempt-token requires --render-attestation")
    );
}

#[test]
fn capture_input_rejects_existing_request_source_without_replacing_it() -> anyhow::Result<()> {
    let source = "classDiagram";
    let fixture = Fixture::new(source)?;
    let attestation = fixture.begin(source)?;
    let artifacts = fixture.receipt.with_extension("artifacts");
    let request_source = artifacts.join("request-source.mmd");
    let protected = b"previous request source";
    fs::write(&request_source, protected)?;
    let input =
        RenderInputFactory::create(DiagramKind::Mermaid, source.into(), serde_json::Value::Null);

    let result = attestation.capture_input(&input);
    assert!(result.as_ref().err().is_some_and(|error| {
        error
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io_error| io_error.kind() == std::io::ErrorKind::AlreadyExists)
    }));
    assert_eq!(fs::read(&request_source)?, protected);
    assert!(!artifacts.join("render-input.json").exists());
    fs::remove_dir_all(fixture.root)?;
    Ok(())
}
