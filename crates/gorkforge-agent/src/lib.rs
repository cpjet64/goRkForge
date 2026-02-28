use async_trait::async_trait;
use gorkforge_core::agent::{Agent, TaskContext, TaskResult, TaskStatus};

pub struct CoreAgent;

#[async_trait]
impl Agent for CoreAgent {
    fn name(&self) -> &'static str {
        "agent"
    }

    async fn run(&self, context: &TaskContext) -> anyhow::Result<TaskResult> {
        Ok(TaskResult {
            status: TaskStatus::Completed,
            output: format!("agent stub ran '{}'", context.task),
        })
    }
}

pub fn new_agent() -> CoreAgent {
    CoreAgent
}
