use async_trait::async_trait;
use gorkforge_core::{Orchestrator, TaskContext, TaskResult, TaskStatus};

pub struct CoreOrchestrator;

#[async_trait]
impl Orchestrator for CoreOrchestrator {
    fn name(&self) -> &'static str {
        "gorkforge-orchestrator"
    }

    async fn coordinate(&self, context: &TaskContext) -> anyhow::Result<TaskResult> {
        Ok(TaskResult {
            status: TaskStatus::Completed,
            output: format!("orchestrator stub for task: {}", context.task),
        })
    }
}

pub fn stub() {
    println!("stub for {}", env!("CARGO_PKG_NAME"));
}
