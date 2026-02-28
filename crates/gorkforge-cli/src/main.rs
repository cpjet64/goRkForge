use clap::{Parser, Subcommand};
use gorkforge_core::{
    agent::reasoner::ReActReasoner, Config, PolicyConfig, Sandbox, TaskContext, ToolSet,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "gorkforge", version, about = "goRkForge CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to policy file
    #[arg(long, default_value = ".gorkforge/policy.toml")]
    policy: String,

    /// Maximum iterations for agent run
    #[arg(long, default_value_t = 8)]
    max_iter: u32,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute task through reasoning loop
    Run {
        /// User task to execute
        task: String,
    },
    /// Invoke full self-improve cycle
    SelfImprove {
        #[arg(long)]
        iterations: Option<u32>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    let config = Config::load()?;
    let policy_path = PathBuf::from(&cli.policy);
    let policy = PolicyConfig::from_file(&policy_path)?;

    let max_iter = match cli.max_iter {
        0 => 1,
        v => policy.max_iterations(v),
    };

    let workspace_root = std::env::current_dir()?;
    let sandbox = Sandbox::new(&workspace_root)?;
    let toolset = ToolSet::new(sandbox, policy);
    let model = config.xai_model.clone();
    let mut reasoner = ReActReasoner::new(config.xai_api_key, model.clone(), toolset, max_iter);

    match cli.command {
        None => {
            println!("goRkForge v0.1 Phase 1  Grok's self-building agent");
            println!("Commands: run <task>, self-improve [--iterations N]");
            Ok(())
        }
        Some(Commands::Run { task }) => {
            let ctx = TaskContext {
                task,
                spec_file: None,
                policy_file: Some(cli.policy),
                max_iter: Some(max_iter),
            };
            let result = reasoner.run_task(&ctx).await?;
            println!("STATUS: {:?}\n{}", result.status, result.output);
            Ok(())
        }
        Some(Commands::SelfImprove { iterations }) => {
            let iters = iterations.unwrap_or(max_iter);
            reasoner.max_iter = iters;
            println!("Self-improving using model: {}", model);
            let result = reasoner.self_improve().await?;
            println!("STATUS: {:?}\n{}", result.status, result.output);
            Ok(())
        }
    }
}
