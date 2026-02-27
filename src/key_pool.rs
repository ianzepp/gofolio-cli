use std::sync::Arc;

use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use crate::agent::client::Provider;

/// Maximum concurrent leases per API key.
const MAX_LEASES_PER_KEY: usize = 4;

/// A pool of API keys for a single provider.
/// Max concurrency = number of keys × MAX_LEASES_PER_KEY.
pub struct KeyPool {
    keys: Vec<String>,
    next: Mutex<usize>,
    semaphore: Arc<Semaphore>,
}

/// RAII guard returned by [`KeyPool::lease`]. The semaphore permit
/// auto-returns when this value is dropped.
pub struct KeyLease {
    pub key: String,
    _permit: OwnedSemaphorePermit,
}

impl KeyPool {
    pub fn new(keys: Vec<String>) -> Self {
        assert!(!keys.is_empty(), "KeyPool requires at least one key");
        let total_permits = keys.len() * MAX_LEASES_PER_KEY;
        let semaphore = Arc::new(Semaphore::new(total_permits));
        Self {
            keys,
            next: Mutex::new(0),
            semaphore,
        }
    }

    /// Lease a key from the pool. Blocks (async) when all permits are in use.
    /// Keys are assigned round-robin across leases.
    pub async fn lease(&self) -> KeyLease {
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore closed");
        let mut idx = self.next.lock().await;
        let key = self.keys[*idx].clone();
        *idx = (*idx + 1) % self.keys.len();
        KeyLease {
            key,
            _permit: permit,
        }
    }

    /// Number of distinct API keys in the pool.
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }

    /// Total max concurrency (keys × leases per key).
    pub fn max_concurrent(&self) -> usize {
        self.keys.len() * MAX_LEASES_PER_KEY
    }
}

/// Load pool API keys for a provider from environment variables.
/// Only checks multi-key env vars (comma-separated plural or numbered).
/// Does NOT fall back to the single-key env var — that's handled by
/// `Config::api_keys_for_provider` so config file `api_keys` arrays
/// take priority over a single env var.
///
/// Resolution order:
/// 1. `ANTHROPIC_API_KEYS` (comma-separated)
/// 2. `ANTHROPIC_API_KEY_1`, `_2`, ... `_20` (stop at first gap)
pub fn load_provider_pool_keys(provider: Provider) -> Vec<String> {
    let base = match provider {
        Provider::Anthropic => "ANTHROPIC_API_KEY",
        Provider::OpenRouter => "OPENROUTER_API_KEY",
        Provider::OpenAI => "OPENAI_API_KEY",
    };

    // 1. Comma-separated plural form
    let plural = format!("{base}S");
    if let Ok(val) = std::env::var(&plural) {
        let keys: Vec<String> = val
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !keys.is_empty() {
            return keys;
        }
    }

    // 2. Numbered: _1, _2, ... _20
    let mut numbered = Vec::new();
    for i in 1..=20 {
        match std::env::var(format!("{base}_{i}")) {
            Ok(v) if !v.trim().is_empty() => numbered.push(v.trim().to_string()),
            _ => break,
        }
    }
    if !numbered.is_empty() {
        return numbered;
    }

    Vec::new()
}
