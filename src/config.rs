use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::agent::client::Provider;

/// Auth sub-object matching the original config.json structure.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Config matching ~/.config/ghostfolio-cli/config.json from the Ink CLI.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traits: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    /// Rust CLI extension — not present in original config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic_api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openrouter_api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai_api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub langchain_api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub langchain_project: Option<String>,
}

impl Config {
    pub fn dir() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".config").join("ghostfolio-cli")
    }

    pub fn path() -> PathBuf {
        Self::dir().join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(config) => config,
                Err(e) => {
                    warn!(
                        path = %path.display(),
                        error = %e,
                        "config: failed to parse config file, using defaults"
                    );
                    Self::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "config: failed to read config file, using defaults"
                );
                Self::default()
            }
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "config: failed to create config directory"
                );
                return;
            }
        }
        let contents = match serde_json::to_string_pretty(self) {
            Ok(contents) => contents + "\n",
            Err(e) => {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "config: failed to serialize config"
                );
                return;
            }
        };
        if let Err(e) = std::fs::write(&path, contents) {
            warn!(
                path = %path.display(),
                error = %e,
                "config: failed to write config file"
            );
        }
    }

    /// Resolve ghostfolio URL: env > config > default.
    pub fn ghostfolio_url(&self) -> String {
        std::env::var("GHOSTFOLIO_URL")
            .ok()
            .or_else(|| self.auth.as_ref().and_then(|a| a.url.clone()))
            .unwrap_or_else(|| "http://localhost:3333".to_string())
    }

    /// Resolve access token: env > config.
    pub fn access_token(&self) -> Option<String> {
        std::env::var("GHOSTFOLIO_ACCESS_TOKEN")
            .ok()
            .or_else(|| self.auth.as_ref().and_then(|a| a.token.clone()))
    }

    /// Resolve Anthropic API key: env > config.
    pub fn anthropic_api_key(&self) -> Option<String> {
        std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .or_else(|| self.anthropic_api_key.clone())
    }

    /// Resolve OpenRouter API key: env > config.
    pub fn openrouter_api_key(&self) -> Option<String> {
        std::env::var("OPENROUTER_API_KEY")
            .ok()
            .or_else(|| self.openrouter_api_key.clone())
    }

    /// Resolve OpenAI API key: env > config.
    pub fn openai_api_key(&self) -> Option<String> {
        std::env::var("OPENAI_API_KEY")
            .ok()
            .or_else(|| self.openai_api_key.clone())
    }

    /// Detect the LLM provider by checking API keys in priority order:
    /// Anthropic > OpenRouter > OpenAI.
    pub fn detect_llm_provider(&self) -> Option<(Provider, String)> {
        if let Some(key) = self.anthropic_api_key() {
            return Some((Provider::Anthropic, key));
        }
        if let Some(key) = self.openrouter_api_key() {
            return Some((Provider::OpenRouter, key));
        }
        if let Some(key) = self.openai_api_key() {
            return Some((Provider::OpenAI, key));
        }
        None
    }

    /// Resolve model: env > config > default.
    pub fn model(&self) -> String {
        std::env::var("GHOSTFOLIO_MODEL")
            .ok()
            .or_else(|| self.model.clone())
            .unwrap_or_else(|| "claude-sonnet-4-6".to_string())
    }

    /// Resolve LangChain/LangSmith API key: env > config.
    pub fn langchain_api_key(&self) -> Option<String> {
        std::env::var("LANGCHAIN_API_KEY")
            .ok()
            .or_else(|| self.langchain_api_key.clone())
    }

    /// Resolve LangChain/LangSmith project name: env > config > default.
    pub fn langchain_project(&self) -> String {
        std::env::var("LANGCHAIN_PROJECT")
            .ok()
            .or_else(|| self.langchain_project.clone())
            .unwrap_or_else(|| "ghostfolio".to_string())
    }

    /// Set auth fields (deep-merge, matching original updateConfig behavior).
    pub fn set_auth(&mut self, url: Option<String>, token: Option<String>) {
        let existing = self.auth.clone().unwrap_or_default();
        self.auth = Some(AuthConfig {
            url: url.or(existing.url),
            token: token.or(existing.token),
        });
    }
}
