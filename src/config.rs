use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let contents = serde_json::to_string_pretty(self).unwrap_or_default() + "\n";
        let _ = std::fs::write(&path, contents);
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

    /// Resolve model: env > config > default.
    pub fn model(&self) -> String {
        std::env::var("GHOSTFOLIO_MODEL")
            .ok()
            .or_else(|| self.model.clone())
            .unwrap_or_else(|| "claude-sonnet-4-6".to_string())
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
