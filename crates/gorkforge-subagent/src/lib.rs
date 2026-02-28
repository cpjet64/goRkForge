use async_trait::async_trait;
use gorkforge_core::agent::{Agent, TaskContext, TaskResult, TaskStatus};

pub struct SubAgent;

#[async_trait]
impl Agent for SubAgent {
    fn name(&self) -> &'static str {
        "subagent"
    }

    async fn run(&self, context: &TaskContext) -> anyhow::Result<TaskResult> {
        Ok(TaskResult {
            status: TaskStatus::Completed,
            output: format!("subagent stub ran '{}'", context.task),
        })
    }
}

pub fn new_subagent() -> SubAgent {
    SubAgent
}
