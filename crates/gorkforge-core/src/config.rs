use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub xai_api_key: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        let _ = dotenvy::dotenv();

        let xai_api_key = std::env::var("xai_api_key")
            .or_else(|_| std::env::var("XAI_API_KEY"))
            .context("missing xai_api_key or XAI_API_KEY in environment")?;

        Ok(Self { xai_api_key })
    }
}
