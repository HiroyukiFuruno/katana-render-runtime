mod commands;
mod diagram_cmd;
mod file_ops;
mod reference_cmd;
mod system;

use clap::Parser;
use commands::{Cli, Commands};
use diagram_cmd::DiagramCommand;
use katana_diagram_renderer::DiagramKind;

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Commands::Mermaid { action } => DiagramCommand::new(DiagramKind::Mermaid).run(action),
        Commands::Drawio { action } => DiagramCommand::new(DiagramKind::Drawio).run(action),
    }
}
