use async_trait::async_trait;
use gorkforge_core::{Agent, TaskContext, TaskResult, TaskStatus};

pub struct SubAgent;

#[async_trait]
impl Agent for SubAgent {
    fn name(&self) -> &'static str {
        "gorkforge-subagent"
    }

    async fn run(&self, context: &TaskContext) -> anyhow::Result<TaskResult> {
        Ok(TaskResult {
            status: TaskStatus::Completed,
            output: format!("subagent stub for task: {}", context.task),
        })
    }
}

pub fn stub() {
    println!("stub for {}", env!("CARGO_PKG_NAME"));
}
