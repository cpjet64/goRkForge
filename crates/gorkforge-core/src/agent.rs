use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct TaskContext {
    pub task: String,
    pub spec_file: Option<String>,
    pub policy_file: Option<String>,
    pub max_iter: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct TaskResult {
    pub status: TaskStatus,
    pub output: String,
}

#[derive(Clone, Debug)]
pub enum TaskStatus {
    Completed,
    Deferred,
    Failed,
}

#[async_trait]
pub trait Agent: Send + Sync {
    fn name(&self) -> &'static str;

    async fn run(&self, context: &TaskContext) -> anyhow::Result<TaskResult>;
}

#[async_trait]
pub trait Orchestrator: Send + Sync {
    fn name(&self) -> &'static str;

    async fn coordinate(&self, context: &TaskContext) -> anyhow::Result<TaskResult>;
}

#[derive(Debug, Default, Clone)]
pub struct MockLlmConfig {
    pub model: String,
}

#[derive(Debug, Default, Clone)]
pub struct MockLlm {
    pub config: MockLlmConfig,
}

impl MockLlm {
    pub fn new() -> Self {
        Self {
            config: MockLlmConfig {
                model: "mock-gpt".to_string(),
            },
        }
    }

    pub async fn infer(&self, prompt: &str) -> String {
        format!("[mock:{}] {}", self.config.model, prompt)
    }
}

#[derive(Default)]
pub struct StubAgent;

#[async_trait]
impl Agent for StubAgent {
    fn name(&self) -> &'static str {
        "stub-agent"
    }

    async fn run(&self, context: &TaskContext) -> anyhow::Result<TaskResult> {
        Ok(TaskResult {
            status: TaskStatus::Completed,
            output: format!(
                "agent stub executed task '{}' with spec={:?} policy={:?} max_iter={:?}",
                context.task, context.spec_file, context.policy_file, context.max_iter
            ),
        })
    }
}
