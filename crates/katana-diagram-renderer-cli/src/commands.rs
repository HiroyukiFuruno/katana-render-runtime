use clap::{Parser, Subcommand};
use std::path::PathBuf;

const DEFAULT_MIN_SCORE: f32 = 99.0;

#[derive(Parser)]
#[command(name = "kdr", version, about = "katana-diagram-renderer CLI")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    Mermaid {
        #[command(subcommand)]
        action: DiagramAction,
    },
    Drawio {
        #[command(subcommand)]
        action: DiagramAction,
    },
}

#[derive(Subcommand)]
pub(crate) enum DiagramAction {
    Render {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        runtime: Option<PathBuf>,
    },
    ReferenceUpdate {
        #[arg(long)]
        fixtures: PathBuf,
    },
    Compare {
        #[arg(long)]
        fixtures: PathBuf,
        #[arg(long, default_value_t = DEFAULT_MIN_SCORE)]
        min_score: f32,
    },
    Bench {
        #[arg(long)]
        fixtures: PathBuf,
    },
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands, DiagramAction};
    use clap::Parser;

    #[test]
    fn parses_mermaid_render_command() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from([
            "kdr", "mermaid", "render", "--input", "in.md", "--output", "out.svg",
        ])?;
        assert!(matches!(
            cli.command,
            Commands::Mermaid {
                action: DiagramAction::Render { .. }
            }
        ));
        Ok(())
    }

    #[test]
    fn parses_drawio_compare_command() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from([
            "kdr",
            "drawio",
            "compare",
            "--fixtures",
            "tests/fixtures/drawio",
            "--min-score",
            "99.5",
        ])?;
        assert!(matches!(
            cli.command,
            Commands::Drawio {
                action: DiagramAction::Compare { .. }
            }
        ));
        Ok(())
    }
}
