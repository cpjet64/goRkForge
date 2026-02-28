use async_trait::async_trait;
use gorkforge_core::{Agent, TaskContext, TaskResult, TaskStatus};

pub struct RemoteAgent;

#[async_trait]
impl Agent for RemoteAgent {
    fn name(&self) -> &'static str {
        "gorkforge-remoteagent"
    }

    async fn run(&self, context: &TaskContext) -> anyhow::Result<TaskResult> {
        Ok(TaskResult {
            status: TaskStatus::Completed,
            output: format!("remoteagent stub for task: {}", context.task),
        })
    }
}

pub fn stub() {
    println!("stub for {}", env!("CARGO_PKG_NAME"));
}
