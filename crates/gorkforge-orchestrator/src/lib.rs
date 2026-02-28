use async_trait::async_trait;
use gorkforge_core::agent::{Orchestrator, TaskContext, TaskResult, TaskStatus};

pub struct CoreOrchestrator;

#[async_trait]
impl Orchestrator for CoreOrchestrator {
    fn name(&self) -> &'static str {
        "orchestrator"
    }

    async fn coordinate(&self, context: &TaskContext) -> anyhow::Result<TaskResult> {
        Ok(TaskResult {
            status: TaskStatus::Completed,
            output: format!("orchestrator stub coordinated '{}'", context.task),
        })
    }
}

pub fn new_orchestrator() -> CoreOrchestrator {
    CoreOrchestrator
}
