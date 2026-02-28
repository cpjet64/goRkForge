use async_trait::async_trait;
use gorkforge_core::platform::Platform;
use gorkforge_core::TaskContext;

pub struct HeadlessPlatform;

#[async_trait]
impl Platform for HeadlessPlatform {
    fn name(&self) -> &'static str {
        "headless"
    }

    async fn execute_task(&self, context: &TaskContext) -> anyhow::Result<String> {
        Ok(format!("headless platform stub: {}", context.task))
    }
}

pub fn new_stub() -> HeadlessPlatform {
    HeadlessPlatform
}
