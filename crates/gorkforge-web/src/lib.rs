use async_trait::async_trait;
use gorkforge_core::platform::Platform;
use gorkforge_core::TaskContext;

pub struct WebPlatform;

#[async_trait]
impl Platform for WebPlatform {
    fn name(&self) -> &'static str {
        "web"
    }

    async fn execute_task(&self, context: &TaskContext) -> anyhow::Result<String> {
        Ok(format!("web platform stub: {}", context.task))
    }
}

pub fn new_stub() -> WebPlatform {
    WebPlatform
}
