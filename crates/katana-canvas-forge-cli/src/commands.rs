use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

const DEFAULT_MIN_SCORE: f32 = 99.0;

#[derive(Parser)]
#[command(name = "kcf", version, about = "katana-canvas-forge CLI")]
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
    Export {
        #[arg(value_enum)]
        format: ExportFormatArg,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    ExportDebug {
        #[arg(long)]
        input: PathBuf,
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

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum ExportFormatArg {
    Html,
    Pdf,
    Png,
    Jpeg,
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn parses_export_html_command() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from([
            "kcf", "export", "html", "--input", "in.html", "--output", "out.html",
        ])?;
        assert!(matches!(cli.command, super::Commands::Export { .. }));
        Ok(())
    }

    #[test]
    fn parses_export_debug_command() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["kcf", "export-debug", "--input", "in.html"])?;
        assert!(matches!(cli.command, super::Commands::ExportDebug { .. }));
        Ok(())
    }
}
