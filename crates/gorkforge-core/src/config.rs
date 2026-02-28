use anyhow::{Context, Result};
use dotenvy::dotenv;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Config {
    pub xai_api_key: String,
    pub xai_model: String,
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    llm: Option<LlmConfig>,
}

#[derive(Debug, Deserialize)]
struct LlmConfig {
    model: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let _ = dotenv().ok();

        let key = env::var("xai_api_key")
            .or_else(|_| env::var("XAI_API_KEY"))
            .context("No xai_api_key found in .env or environment (xai_api_key / XAI_API_KEY)")?;
        let model = Self::load_model().unwrap_or(None).unwrap_or_else(|| {
            env::var("XAI_MODEL").unwrap_or_else(|_| "grok-4-1-fast-reasoning".to_string())
        });

        tracing::info!(" API key loaded successfully (length: {})", key.len());
        println!(" API key loaded successfully (length: {})", key.len());
        tracing::info!(" LLM model configured: {}", model);
        println!(" LLM model configured: {}", model);

        Ok(Config {
            xai_api_key: key,
            xai_model: model,
        })
    }

    fn load_model() -> Result<Option<String>> {
        let candidate = Path::new("gorkforge.config.toml");
        if !candidate.exists() {
            return Ok(None);
        }

        let text = fs::read_to_string(candidate)
            .context("read gorkforge.config.toml for model configuration")?;
        let cfg: FileConfig = toml::from_str(&text).context("parse gorkforge.config.toml")?;

        Ok(cfg
            .llm
            .and_then(|l| l.model)
            .or_else(|| env::var("XAI_MODEL").ok()))
    }
}
