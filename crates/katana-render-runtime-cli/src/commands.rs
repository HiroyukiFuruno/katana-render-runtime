use clap::{Parser, Subcommand, ValueEnum};
use katana_render_runtime::PlantUmlThemeCatalog;
use std::path::PathBuf;

const DEFAULT_MIN_SCORE: f32 = 99.0;

#[derive(Parser)]
#[command(
    name = "krr",
    bin_name = "krr",
    version,
    about = "katana-render-runtime CLI"
)]
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
    Plantuml {
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
        output: Option<PathBuf>,
        #[arg(long)]
        runtime: Option<PathBuf>,
        #[arg(long, long_help = PlantUmlThemeCatalog::HELP_TEXT)]
        theme: Option<String>,
        #[arg(long = "theme-from")]
        theme_from: Option<String>,
        #[arg(long = "theme-mode", value_enum)]
        theme_mode: Option<ThemeModeArg>,
        #[arg(long = "cache-dir")]
        cache_dir: Option<PathBuf>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ThemeModeArg {
    Dark,
    Light,
}

impl ThemeModeArg {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands, DiagramAction, ThemeModeArg};
    use clap::Parser;

    #[test]
    fn parses_mermaid_render_command() -> Result<(), Box<dyn std::error::Error>> {
        let args = "krr mermaid render --input in.md --output out.svg";
        let cli = Cli::try_parse_from(args.split_whitespace())?;
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
        let args = "krr drawio compare --fixtures tests/fixtures/drawio --min-score 99.5";
        let cli = Cli::try_parse_from(args.split_whitespace())?;
        assert!(matches!(
            cli.command,
            Commands::Drawio {
                action: DiagramAction::Compare { .. }
            }
        ));
        Ok(())
    }

    #[test]
    fn parses_reference_update_command() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from([
            "krr",
            "mermaid",
            "reference-update",
            "--fixtures",
            "tests/fixtures/mermaid",
        ])?;

        assert!(matches!(
            cli.command,
            Commands::Mermaid {
                action: DiagramAction::ReferenceUpdate { .. }
            }
        ));
        Ok(())
    }

    #[test]
    fn parses_bench_command() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from([
            "krr",
            "plantuml",
            "bench",
            "--fixtures",
            "tests/fixtures/plantuml",
        ])?;

        assert!(matches!(
            cli.command,
            Commands::Plantuml {
                action: DiagramAction::Bench { .. }
            }
        ));
        Ok(())
    }

    #[test]
    fn compare_command_uses_default_minimum_score() -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from([
            "krr",
            "drawio",
            "compare",
            "--fixtures",
            "tests/fixtures/drawio",
        ])?;

        assert!(matches!(
            cli.command,
            Commands::Drawio {
                action: DiagramAction::Compare { min_score, .. }
            } if (min_score - 99.0).abs() < f32::EPSILON
        ));
        Ok(())
    }

    #[test]
    fn parses_plantuml_render_command() -> Result<(), Box<dyn std::error::Error>> {
        let args = "krr plantuml render --input in.puml --theme cyborg --theme-mode light";
        let cli = Cli::try_parse_from(args.split_whitespace())?;
        assert!(matches!(
            cli.command,
            Commands::Plantuml {
                action: DiagramAction::Render {
                    theme: Some(ref theme),
                    theme_mode: Some(ThemeModeArg::Light),
                    ..
                }
            } if theme == "cyborg"
        ));
        Ok(())
    }

    #[test]
    fn parses_plantuml_cache_dir() -> Result<(), Box<dyn std::error::Error>> {
        let args = "krr plantuml render --input in.puml --cache-dir /tmp/krr-cache";
        let cli = Cli::try_parse_from(args.split_whitespace())?;
        assert!(matches!(
            cli.command,
            Commands::Plantuml {
                action: DiagramAction::Render {
                    cache_dir: Some(ref path),
                    ..
                }
            } if path == std::path::Path::new("/tmp/krr-cache")
        ));
        Ok(())
    }

    #[test]
    fn plantuml_render_help_lists_theme_options() -> Result<(), Box<dyn std::error::Error>> {
        let result = Cli::try_parse_from(["krr", "plantuml", "render", "--help"]);
        let help = result
            .err()
            .map_or_else(String::new, |error| error.to_string());

        assert!(help.contains("--theme <THEME>"), "{help}");
        assert!(help.contains("cyborg"), "{help}");
        assert!(help.contains("black-knight"), "{help}");
        assert!(help.contains("--theme-mode <THEME_MODE>"), "{help}");
        assert!(help.contains("[possible values: dark, light]"), "{help}");
        assert!(help.contains("--cache-dir <CACHE_DIR>"), "{help}");
        Ok(())
    }

    #[test]
    fn help_uses_canonical_krr_name_when_executable_has_extension()
    -> Result<(), Box<dyn std::error::Error>> {
        let result = Cli::try_parse_from(["krr.exe", "--help"]);
        let help = result
            .err()
            .map_or_else(String::new, |error| error.to_string());

        assert!(help.contains("Usage: krr <COMMAND>"), "{help}");
        assert!(!help.contains("Usage: krr.exe <COMMAND>"), "{help}");
        Ok(())
    }

    #[test]
    fn theme_mode_values_have_canonical_names() {
        assert_eq!(ThemeModeArg::Dark.as_str(), "dark");
        assert_eq!(ThemeModeArg::Light.as_str(), "light");
    }
}
