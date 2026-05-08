mod commands;
mod diagram_cmd;
mod export_cmd;
mod export_debug_cmd;
mod file_ops;
mod reference_cmd;
mod system;

use clap::Parser;
use commands::{Cli, Commands};
use diagram_cmd::DiagramCommand;
use export_cmd::ExportCommand;
use export_debug_cmd::ExportDebugCommand;
use katana_canvas_forge::DiagramKind;

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Commands::Mermaid { action } => DiagramCommand::new(DiagramKind::Mermaid).run(action),
        Commands::Drawio { action } => DiagramCommand::new(DiagramKind::Drawio).run(action),
        Commands::Export {
            format,
            input,
            output,
        } => ExportCommand::new(format, input, output).run(),
        Commands::ExportDebug { input } => ExportDebugCommand::new(input).run(),
    }
}
