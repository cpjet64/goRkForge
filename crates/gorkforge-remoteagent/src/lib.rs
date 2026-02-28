use async_trait::async_trait;
use gorkforge_core::agent::{Agent, TaskContext, TaskResult, TaskStatus};

pub struct RemoteAgent;

#[async_trait]
impl Agent for RemoteAgent {
    fn name(&self) -> &'static str {
        "remoteagent"
    }

    async fn run(&self, context: &TaskContext) -> anyhow::Result<TaskResult> {
        Ok(TaskResult {
            status: TaskStatus::Completed,
            output: format!("remoteagent stub ran '{}'", context.task),
        })
    }
}

pub fn new_remote_agent() -> RemoteAgent {
    RemoteAgent
}
