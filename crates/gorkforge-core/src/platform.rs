use crate::agent::{TaskContext, TaskResult};
use async_trait::async_trait;

#[async_trait]
pub trait Platform: Send + Sync {
    fn name(&self) -> &'static str;

    async fn execute_task(&self, context: &TaskContext) -> anyhow::Result<TaskResult>;
}
