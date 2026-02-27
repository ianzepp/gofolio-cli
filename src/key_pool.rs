use std::sync::Arc;

use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use crate::agent::client::Provider;

/// A pool of API keys for a single provider.
/// Max concurrency equals the number of keys — natural backpressure.
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
        let semaphore = Arc::new(Semaphore::new(keys.len()));
        Self {
            keys,
            next: Mutex::new(0),
            semaphore,
        }
    }

    /// Lease a key from the pool. Blocks (async) when all keys are in use.
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

    pub fn pool_size(&self) -> usize {
        self.keys.len()
    }
}

/// Load all API keys for a provider from environment variables.
///
/// Resolution order:
/// 1. `ANTHROPIC_API_KEYS` (comma-separated)
/// 2. `ANTHROPIC_API_KEY_1`, `_2`, ... `_20` (stop at first gap)
/// 3. Single `ANTHROPIC_API_KEY` (fallback, pool of size 1)
pub fn load_provider_keys(provider: Provider) -> Vec<String> {
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

    // 3. Single key fallback
    if let Ok(v) = std::env::var(base) {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return vec![v];
        }
    }

    Vec::new()
}
