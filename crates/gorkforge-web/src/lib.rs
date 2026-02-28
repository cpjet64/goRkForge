use async_trait::async_trait;
use gorkforge_core::{Platform, TaskContext, TaskResult, TaskStatus};

pub struct WebPlatform;

#[async_trait]
impl Platform for WebPlatform {
    fn name(&self) -> &'static str {
        "gorkforge-web"
    }

    async fn execute_task(&self, context: &TaskContext) -> anyhow::Result<TaskResult> {
        Ok(TaskResult {
            status: TaskStatus::Completed,
            output: format!("web platform stub for task: {}", context.task),
        })
    }
}

pub fn stub() {
    println!("stub for {}", env!("CARGO_PKG_NAME"));
}
