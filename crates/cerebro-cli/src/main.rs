use anyhow::Result;
use cerebro::{CerebroCortex, types::{MemoryType, VisibilityScope}};
use clap::{Parser, Subcommand};

/// cerebro — CerebroCortex command-line interface
/// Mirrors Python interfaces/cli.py (click → clap).
#[derive(Parser)]
#[command(name = "cerebro", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Store a new memory
    Remember {
        content:     String,
        #[arg(long, default_value = "semantic")]
        memory_type: String,
    },
    /// Search memories
    Recall {
        query:     String,
        #[arg(short, long, default_value = "10")]
        top_k:     usize,
    },
    /// Show database statistics
    Stats,
    /// Run a dream consolidation cycle
    Dream,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli    = Cli::parse();
    let config = cerebro::config::Config::from_env()?;
    let brain  = CerebroCortex::new(config).await?;

    match cli.command {
        Command::Remember { content, memory_type } => {
            let mt = serde_json::from_value(serde_json::Value::String(memory_type))
                .unwrap_or(MemoryType::Semantic);
            let node = brain.remember(content, mt, VisibilityScope::global()).await?;
            println!("stored: {}", node.id.0);
        }
        Command::Recall { query, top_k } => {
            let results = brain.recall(&query, top_k, VisibilityScope::global()).await?;
            if results.is_empty() {
                println!("no results");
            }
            for (node, score) in results {
                println!("[{score:.3}] {}: {}", node.id.0, node.content);
            }
        }
        Command::Stats => {
            // TODO: step 11
            println!("stats not yet implemented");
        }
        Command::Dream => {
            // TODO: step 10
            println!("dream not yet implemented");
        }
    }
    Ok(())
}
