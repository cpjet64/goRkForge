use clap::{Parser, Subcommand};
use gorkforge_core::Config;
use tracing::info;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a task (Phase 0 stub)
    Run {
        task: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Load config (this prints the  message)
    let _config = Config::load()?;

    match cli.command {
        Some(Commands::Run { task }) => {
            info!("🚀 Running task: {}", task);
            println!(" goRkForge Phase 0 is alive!\nTask received: {}", task);
            println!("(Full agent loop coming in Phase 1  run \'gorkforge self-improve\' after upgrade)");
        }
        None => {
            println!("goRkForge v0.1.0  Grok's self-building agent");
            println!("Usage: gorkforge run \"your task here\"");
        }
    }

    Ok(())
}
