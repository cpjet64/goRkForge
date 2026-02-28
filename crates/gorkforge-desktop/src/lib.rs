use async_trait::async_trait;
use gorkforge_core::platform::Platform;
use gorkforge_core::TaskContext;

pub struct DesktopPlatform;

#[async_trait]
impl Platform for DesktopPlatform {
    fn name(&self) -> &'static str {
        "desktop"
    }

    async fn execute_task(&self, context: &TaskContext) -> anyhow::Result<String> {
        Ok(format!("desktop platform stub: {}", context.task))
    }
}

pub fn new_stub() -> DesktopPlatform {
    DesktopPlatform
}
