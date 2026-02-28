use anyhow::Result;
use dotenvy::dotenv;
use std::env;

#[derive(Debug)]
pub struct Config {
    pub xai_api_key: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        let _ = dotenv(); // ignore if no .env
        let key = env::var("xai_api_key")
            .or_else(|_| env::var("XAI_API_KEY"))
            .map_err(|_| anyhow::anyhow!("No xai_api_key found in .env or env vars"))?;

        tracing::info!(" API key loaded successfully (length: {})", key.len());
        Ok(Config { xai_api_key: key })
    }
}
