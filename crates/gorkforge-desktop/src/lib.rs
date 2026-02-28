use async_trait::async_trait;
use gorkforge_core::{Platform, TaskContext, TaskResult, TaskStatus};

pub struct DesktopPlatform;

#[async_trait]
impl Platform for DesktopPlatform {
    fn name(&self) -> &'static str {
        "gorkforge-desktop"
    }

    async fn execute_task(&self, context: &TaskContext) -> anyhow::Result<TaskResult> {
        Ok(TaskResult {
            status: TaskStatus::Completed,
            output: format!("desktop platform stub for task: {}", context.task),
        })
    }
}

pub fn stub() {
    println!("stub for {}", env!("CARGO_PKG_NAME"));
}
