use async_trait::async_trait;
use crate::agent::TaskContext;
use anyhow::Result;

#[async_trait]
pub trait Platform: Send + Sync {
    fn name(&self) -> &'static str;

    async fn execute_task(&self, context: &TaskContext) -> Result<String>;
}
