use clap::{Parser, Subcommand};

const DEFAULT_MIN_SCORE: f32 = 99.0;

#[derive(Parser)]
#[command(name = "kcf", version, about = "katana-canvas-forge CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Mermaid {
        #[command(subcommand)]
        action: MermaidAction,
    },
}

#[derive(Subcommand)]
enum MermaidAction {
    Render {
        #[arg(long)]
        input: String,
        #[arg(long)]
        output: String,
    },
    ReferenceUpdate {
        #[arg(long)]
        fixtures: String,
    },
    Compare {
        #[arg(long)]
        fixtures: String,
        #[arg(long, default_value_t = DEFAULT_MIN_SCORE)]
        min_score: f32,
    },
    Bench {
        #[arg(long)]
        fixtures: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Mermaid { action } => match action {
            MermaidAction::Render { input, output } => {
                println!("[scaffold] mermaid render {input} -> {output}");
            }
            MermaidAction::ReferenceUpdate { fixtures } => {
                println!("[scaffold] mermaid reference-update {fixtures}");
            }
            MermaidAction::Compare {
                fixtures,
                min_score,
            } => {
                println!("[scaffold] mermaid compare {fixtures} min-score={min_score}");
            }
            MermaidAction::Bench { fixtures } => {
                println!("[scaffold] mermaid bench {fixtures}");
            }
        },
    }
    Ok(())
}
