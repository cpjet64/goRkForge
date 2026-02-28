pub mod agent;
pub mod config;
pub mod platform;

pub use agent::{Agent, MockLlm, MockLlmConfig, Orchestrator, TaskContext, TaskResult, TaskStatus};
pub use config::Config;
pub use platform::Platform;

#[derive(Clone, Debug)]
pub struct CoreConfig {
    pub policy_file: Option<String>,
    pub spec_file: Option<String>,
    pub max_iter: Option<u32>,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            policy_file: None,
            spec_file: None,
            max_iter: None,
        }
    }
}

pub async fn run_with_platform(
    platform: &dyn Platform,
    task: String,
    spec: Option<String>,
    policy: Option<String>,
    max_iter: Option<u32>,
) -> anyhow::Result<String> {
    let context = TaskContext {
        task,
        spec_file: spec,
        policy_file: policy,
        max_iter,
    };
    platform.execute_task(&context).await
}
