use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::agent::client::{
    Adapter, Provider, ProviderConfig, default_model_for_provider, provider_from_id,
};

#[derive(Debug, Clone)]
pub enum KeyFormatStatus {
    Expected,
    LooksLike(Provider),
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ProviderKeyStatus {
    pub provider: Provider,
    pub configured: bool,
    pub source: Option<String>,
    pub format: Option<KeyFormatStatus>,
}

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
    pub model_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traits: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_adapter: Option<String>,
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
    pub fn provider_env_var(provider: Provider) -> &'static str {
        match provider {
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::OpenRouter => "OPENROUTER_API_KEY",
            Provider::OpenAI => "OPENAI_API_KEY",
        }
    }

    fn looks_like_provider(key: &str) -> Option<Provider> {
        if key.starts_with("sk-ant-") {
            return Some(Provider::Anthropic);
        }
        if key.starts_with("sk-or-v1-") || key.starts_with("sk-or-") {
            return Some(Provider::OpenRouter);
        }
        if key.starts_with("sk-proj-") || key.starts_with("sk-") {
            return Some(Provider::OpenAI);
        }
        None
    }

    fn key_for_provider_with_source(&self, provider: Provider) -> Option<(String, String)> {
        let env_name = Self::provider_env_var(provider);
        if let Ok(v) = std::env::var(env_name)
            && !v.trim().is_empty()
        {
            return Some((v, format!("env:{env_name}")));
        }
        let cfg_val = match provider {
            Provider::Anthropic => self.anthropic_api_key.clone(),
            Provider::OpenRouter => self.openrouter_api_key.clone(),
            Provider::OpenAI => self.openai_api_key.clone(),
        };
        cfg_val
            .filter(|v| !v.trim().is_empty())
            .map(|v| (v, "config".to_string()))
    }

    pub fn provider_key_statuses(&self) -> Vec<ProviderKeyStatus> {
        let mut out = Vec::new();
        for provider in [Provider::Anthropic, Provider::OpenRouter, Provider::OpenAI] {
            let status = if let Some((key, source)) = self.key_for_provider_with_source(provider) {
                let format = match Self::looks_like_provider(&key) {
                    Some(detected) if detected == provider => KeyFormatStatus::Expected,
                    Some(other) => KeyFormatStatus::LooksLike(other),
                    None => KeyFormatStatus::Unknown,
                };
                ProviderKeyStatus {
                    provider,
                    configured: true,
                    source: Some(source),
                    format: Some(format),
                }
            } else {
                ProviderKeyStatus {
                    provider,
                    configured: false,
                    source: None,
                    format: None,
                }
            };
            out.push(status);
        }
        out
    }

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
            .filter(|s| !s.trim().is_empty())
            .or_else(|| self.anthropic_api_key.clone())
            .filter(|s| !s.trim().is_empty())
    }

    /// Resolve OpenRouter API key: env > config.
    pub fn openrouter_api_key(&self) -> Option<String> {
        std::env::var("OPENROUTER_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| self.openrouter_api_key.clone())
            .filter(|s| !s.trim().is_empty())
    }

    /// Resolve OpenAI API key: env > config.
    pub fn openai_api_key(&self) -> Option<String> {
        std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| self.openai_api_key.clone())
            .filter(|s| !s.trim().is_empty())
    }

    fn openai_adapter(&self) -> Adapter {
        let value = std::env::var("OPENAI_ADAPTER")
            .ok()
            .or_else(|| self.llm_adapter.clone());
        value
            .as_deref()
            .and_then(Adapter::parse)
            .unwrap_or(Adapter::OpenAIChatCompletions)
    }

    fn openrouter_adapter(&self) -> Adapter {
        let value = std::env::var("OPENROUTER_ADAPTER")
            .ok()
            .or_else(|| self.llm_adapter.clone());
        value
            .as_deref()
            .and_then(Adapter::parse)
            .unwrap_or(Adapter::OpenAIChatCompletions)
    }

    pub fn configured_llm_providers(&self) -> Vec<ProviderConfig> {
        let mut providers = Vec::new();
        if let Some(api_key) = self.anthropic_api_key() {
            providers.push(ProviderConfig {
                provider: Provider::Anthropic,
                adapter: Adapter::AnthropicMessages,
                api_key,
            });
        }
        if let Some(api_key) = self.openrouter_api_key() {
            providers.push(ProviderConfig {
                provider: Provider::OpenRouter,
                adapter: self.openrouter_adapter(),
                api_key,
            });
        }
        if let Some(api_key) = self.openai_api_key() {
            providers.push(ProviderConfig {
                provider: Provider::OpenAI,
                adapter: self.openai_adapter(),
                api_key,
            });
        }
        providers
    }

    pub fn preferred_llm_provider(&self, configured: &[ProviderConfig]) -> Option<Provider> {
        let preferred = std::env::var("GHOSTFOLIO_LLM_PROVIDER")
            .ok()
            .or_else(|| self.llm_provider.clone())
            .and_then(|id| provider_from_id(id.trim().to_lowercase().as_str()));

        if let Some(provider) = preferred
            && configured.iter().any(|c| c.provider == provider)
        {
            return Some(provider);
        }

        configured.first().map(|c| c.provider)
    }

    /// Resolve model: env > config > provider-aware default.
    pub fn model_for_provider(&self, provider: Provider) -> String {
        if let Ok(model) = std::env::var("GHOSTFOLIO_MODEL") {
            return model;
        }

        let same_provider = self
            .model_provider
            .as_deref()
            .and_then(provider_from_id)
            .map(|p| p == provider)
            .unwrap_or(false);

        if same_provider && let Some(model) = self.model.clone() {
            return model;
        }

        default_model_for_provider(provider).to_string()
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
