use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::agent::client::{ModelEntry, Provider};
use crate::config::Config;

#[derive(Debug, Serialize, Deserialize)]
struct ProviderModelsCache {
    provider: String,
    updated_at: String,
    models: Vec<ModelEntry>,
}

fn providers_dir() -> PathBuf {
    Config::dir().join("providers")
}

fn cache_path(provider: Provider) -> PathBuf {
    providers_dir().join(format!("{}.json", provider.id()))
}

pub fn load(provider: Provider) -> Option<Vec<ModelEntry>> {
    let path = cache_path(provider);
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(_) => return None,
    };
    match serde_json::from_str::<ProviderModelsCache>(&contents) {
        Ok(cache) => Some(cache.models),
        Err(e) => {
            warn!(
                path = %path.display(),
                error = %e,
                "provider-cache: failed to parse model cache"
            );
            None
        }
    }
}

pub fn save(provider: Provider, models: &[ModelEntry]) {
    let path = cache_path(provider);
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        warn!(
            path = %path.display(),
            error = %e,
            "provider-cache: failed to create providers directory"
        );
        return;
    }

    let payload = ProviderModelsCache {
        provider: provider.id().to_string(),
        updated_at: Utc::now().to_rfc3339(),
        models: models.to_vec(),
    };

    let json = match serde_json::to_string_pretty(&payload) {
        Ok(j) => j + "\n",
        Err(e) => {
            warn!(error = %e, "provider-cache: failed to serialize model cache");
            return;
        }
    };

    if let Err(e) = std::fs::write(&path, json) {
        warn!(
            path = %path.display(),
            error = %e,
            "provider-cache: failed to write model cache"
        );
    }
}
