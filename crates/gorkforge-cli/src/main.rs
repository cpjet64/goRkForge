use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use gorkforge_core::config::Config;
use gorkforge_core::platform::Platform;
use gorkforge_core::{run_with_platform, CoreConfig, TaskContext};
use tracing::{info, Level};

#[derive(Parser)]
#[command(name = "gorkforge")]
#[command(about = "Groks self-improving coding agent")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a task through the core runtime
    Run {
        /// Task text to run
        task: String,

        /// Optional spec file path
        #[arg(long)]
        spec: Option<String>,

        /// Optional policy file path
        #[arg(long)]
        policy: Option<String>,

        /// Maximum runtime iterations
        #[arg(long = "max-iter")]
        max_iter: Option<u32>,
    },
}

#[derive(Default)]
struct CliPlatform;

#[async_trait::async_trait]
impl Platform for CliPlatform {
    fn name(&self) -> &'static str {
        "cli"
    }

    async fn execute_task(&self, context: &TaskContext) -> Result<String> {
        info!(
            task = %context.task,
            spec = ?context.spec_file,
            policy = ?context.policy_file,
            max_iter = ?context.max_iter,
            "running task from cli"
        );

        let llm = gorkforge_core::MockLlm::new();
        let message = llm.infer(&format!("Echo request received: {}", context.task)).await;
        let iter_line = match context.max_iter {
            Some(v) => format!("max-iter set to {}", v),
            None => "using default iteration policy".to_string(),
        };

        Ok(format!("{}\nDone: {}", message, iter_line))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let config = Config::load()?;
    println!(" API key loaded successfully (key length: {})", config.xai_api_key.len());

    let cli = Cli::parse();
    let _config = CoreConfig::default();

    match cli.command {
        Some(Commands::Run {
            task,
            spec,
            policy,
            max_iter,
        }) => {
            let out = run_with_platform(&CliPlatform, task, spec, policy, max_iter).await?;
            println!("{}", out);
            Ok(())
        }
        None => {
            println!("{}", Cli::command().render_help());
            Ok(())
        }
    }
}
