use std::path::{Path, PathBuf};

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

/// Auth section.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Per-provider section with single key and key pool.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProviderKeys {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub api_keys: Vec<String>,
}

/// LangChain/LangSmith section.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LangChainConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// Main config — `~/.config/ghostfolio-cli/config.toml`
///
/// ```toml
/// [auth]
/// url = "http://localhost:3333"
/// token = "..."
///
/// model = "claude-sonnet-4-6"
/// llm_provider = "anthropic"
///
/// [anthropic]
/// api_keys = [
///   "sk-ant-key1",
///   "sk-ant-key2",
/// ]
///
/// [openrouter]
/// api_key = "sk-or-..."
///
/// [langchain]
/// api_key = "..."
/// project = "ghostfolio"
/// ```
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traits: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_adapter: Option<String>,

    #[serde(default, skip_serializing_if = "is_provider_keys_empty")]
    pub anthropic: ProviderKeys,
    #[serde(default, skip_serializing_if = "is_provider_keys_empty")]
    pub openrouter: ProviderKeys,
    #[serde(default, skip_serializing_if = "is_provider_keys_empty")]
    pub openai: ProviderKeys,

    #[serde(default, skip_serializing_if = "is_langchain_empty")]
    pub langchain: LangChainConfig,
}

fn is_provider_keys_empty(pk: &ProviderKeys) -> bool {
    pk.api_key.is_none() && pk.api_keys.is_empty()
}

fn is_langchain_empty(lc: &LangChainConfig) -> bool {
    lc.api_key.is_none() && lc.project.is_none()
}

// ---------------------------------------------------------------------------
// Legacy JSON config for migration
// ---------------------------------------------------------------------------

/// Legacy flat JSON config — used only for one-time migration.
#[derive(Debug, Default, Deserialize)]
struct LegacyConfig {
    auth: Option<AuthConfig>,
    model: Option<String>,
    model_provider: Option<String>,
    traits: Option<Vec<String>>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    llm_provider: Option<String>,
    llm_adapter: Option<String>,
    anthropic_api_key: Option<String>,
    openrouter_api_key: Option<String>,
    openai_api_key: Option<String>,
    langchain_api_key: Option<String>,
    langchain_project: Option<String>,
}

impl From<LegacyConfig> for Config {
    fn from(old: LegacyConfig) -> Self {
        Self {
            auth: old.auth,
            model: old.model,
            model_provider: old.model_provider,
            traits: old.traits,
            session_id: old.session_id,
            llm_provider: old.llm_provider,
            llm_adapter: old.llm_adapter,
            anthropic: ProviderKeys {
                api_key: old.anthropic_api_key,
                api_keys: Vec::new(),
            },
            openrouter: ProviderKeys {
                api_key: old.openrouter_api_key,
                api_keys: Vec::new(),
            },
            openai: ProviderKeys {
                api_key: old.openai_api_key,
                api_keys: Vec::new(),
            },
            langchain: LangChainConfig {
                api_key: old.langchain_api_key,
                project: old.langchain_project,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Config impl
// ---------------------------------------------------------------------------

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

    fn provider_keys(&self, provider: Provider) -> &ProviderKeys {
        match provider {
            Provider::Anthropic => &self.anthropic,
            Provider::OpenRouter => &self.openrouter,
            Provider::OpenAI => &self.openai,
        }
    }

    fn provider_keys_mut(&mut self, provider: Provider) -> &mut ProviderKeys {
        match provider {
            Provider::Anthropic => &mut self.anthropic,
            Provider::OpenRouter => &mut self.openrouter,
            Provider::OpenAI => &mut self.openai,
        }
    }

    fn key_for_provider_with_source(&self, provider: Provider) -> Option<(String, String)> {
        let env_name = Self::provider_env_var(provider);
        if let Ok(v) = std::env::var(env_name)
            && !v.trim().is_empty()
        {
            return Some((v, format!("env:{env_name}")));
        }
        let pk = self.provider_keys(provider);
        // Prefer first key from the pool, fall back to single key
        if let Some(first) = pk.api_keys.first().filter(|s| !s.trim().is_empty()) {
            return Some((first.clone(), "config:api_keys[0]".to_string()));
        }
        pk.api_key
            .clone()
            .filter(|v| !v.trim().is_empty())
            .map(|v| (v, "config:api_key".to_string()))
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
        Self::dir().join("config.toml")
    }

    fn legacy_path() -> PathBuf {
        Self::dir().join("config.json")
    }

    pub fn load() -> Self {
        let toml_path = Self::path();
        let json_path = Self::legacy_path();

        // Try TOML first
        if let Ok(contents) = std::fs::read_to_string(&toml_path) {
            match toml::from_str(&contents) {
                Ok(config) => return config,
                Err(e) => {
                    warn!(
                        path = %toml_path.display(),
                        error = %e,
                        "config: failed to parse config.toml, using defaults"
                    );
                    return Self::default();
                }
            }
        }

        // Fall back to legacy JSON and auto-migrate
        match std::fs::read_to_string(&json_path) {
            Ok(contents) => match serde_json::from_str::<LegacyConfig>(&contents) {
                Ok(legacy) => {
                    let config: Config = legacy.into();
                    // Auto-migrate: save as TOML
                    config.save();
                    eprintln!(
                        "config: migrated {} -> {}",
                        json_path.display(),
                        toml_path.display()
                    );
                    config
                }
                Err(e) => {
                    warn!(
                        path = %json_path.display(),
                        error = %e,
                        "config: failed to parse legacy config.json, using defaults"
                    );
                    Self::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => {
                warn!(
                    path = %json_path.display(),
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
        let contents = match toml::to_string_pretty(self) {
            Ok(contents) => contents,
            Err(e) => {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "config: failed to serialize config"
                );
                return;
            }
        };
        if let Err(e) = write_config_file(&path, &contents) {
            warn!(
                path = %path.display(),
                error = %e,
                "config: failed to write config file"
            );
            return;
        }

        if let Err(e) = enforce_private_permissions(&path) {
            warn!(
                path = %path.display(),
                error = %e,
                "config: failed to enforce private config permissions"
            );
        }
    }

    /// Resolve ghostfolio URL: env > config > default.
    pub fn ghostfolio_url(&self) -> String {
        env_var("GHOSTFOLIO_URL")
            .or_else(|| self.auth.as_ref().and_then(|a| a.url.clone()))
            .unwrap_or_else(|| "http://localhost:3333".to_string())
    }

    /// Resolve access token: env > config.
    pub fn access_token(&self) -> Option<String> {
        env_var("GHOSTFOLIO_ACCESS_TOKEN")
            .or_else(|| self.auth.as_ref().and_then(|a| a.token.clone()))
    }

    /// Resolve single Anthropic API key: env > config.
    pub fn anthropic_api_key(&self) -> Option<String> {
        env_var_non_empty("ANTHROPIC_API_KEY")
            .or_else(|| {
                self.anthropic
                    .api_keys
                    .first()
                    .cloned()
                    .or_else(|| self.anthropic.api_key.clone())
            })
            .filter(|s| !s.trim().is_empty())
    }

    /// Resolve single OpenRouter API key: env > config.
    pub fn openrouter_api_key(&self) -> Option<String> {
        env_var_non_empty("OPENROUTER_API_KEY")
            .or_else(|| {
                self.openrouter
                    .api_keys
                    .first()
                    .cloned()
                    .or_else(|| self.openrouter.api_key.clone())
            })
            .filter(|s| !s.trim().is_empty())
    }

    /// Resolve single OpenAI API key: env > config.
    pub fn openai_api_key(&self) -> Option<String> {
        env_var_non_empty("OPENAI_API_KEY")
            .or_else(|| {
                self.openai
                    .api_keys
                    .first()
                    .cloned()
                    .or_else(|| self.openai.api_key.clone())
            })
            .filter(|s| !s.trim().is_empty())
    }

    fn openai_adapter(&self) -> Adapter {
        let value = env_var("OPENAI_ADAPTER").or_else(|| self.llm_adapter.clone());
        value
            .as_deref()
            .and_then(Adapter::parse)
            .unwrap_or(Adapter::OpenAIChatCompletions)
    }

    fn openrouter_adapter(&self) -> Adapter {
        let value = env_var("OPENROUTER_ADAPTER").or_else(|| self.llm_adapter.clone());
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
        let preferred = env_var("GHOSTFOLIO_LLM_PROVIDER")
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

    /// Load all API keys for a provider (pool mode).
    /// Resolution: env pool vars > config `api_keys` array > config `api_key` single > env single.
    pub fn api_keys_for_provider(&self, provider: Provider) -> Vec<String> {
        // 1. Env pool keys (comma-separated or numbered)
        let env_keys = crate::key_pool::load_provider_keys(provider);
        if !env_keys.is_empty() {
            return env_keys;
        }
        // 2. Config api_keys array
        let pk = self.provider_keys(provider);
        let pool: Vec<String> = pk
            .api_keys
            .iter()
            .filter(|s| !s.trim().is_empty())
            .cloned()
            .collect();
        if !pool.is_empty() {
            return pool;
        }
        // 3. Config single api_key
        if let Some(ref key) = pk.api_key {
            if !key.trim().is_empty() {
                return vec![key.clone()];
            }
        }
        Vec::new()
    }

    /// Resolve LangChain/LangSmith API key: env > config.
    pub fn langchain_api_key(&self) -> Option<String> {
        env_var("LANGCHAIN_API_KEY").or_else(|| self.langchain.api_key.clone())
    }

    /// Resolve LangChain/LangSmith project name: env > config > default.
    pub fn langchain_project(&self) -> String {
        env_var("LANGCHAIN_PROJECT")
            .or_else(|| self.langchain.project.clone())
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

fn env_var(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) => Some(value),
        Err(_) => None,
    }
}

fn env_var_non_empty(name: &str) -> Option<String> {
    env_var(name).filter(|s| !s.trim().is_empty())
}

#[cfg(unix)]
fn write_config_file(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn write_config_file(path: &Path, contents: &str) -> std::io::Result<()> {
    std::fs::write(path, contents)
}

#[cfg(unix)]
fn enforce_private_permissions(path: &Path) -> std::io::Result<()> {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn enforce_private_permissions(_path: &Path) -> std::io::Result<()> {
    Ok(())
}
