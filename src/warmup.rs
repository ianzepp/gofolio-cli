use tokio::sync::oneshot;
use tracing::{info, warn};

use crate::api::GhostfolioClient;

/// Pre-fetched portfolio context for the LLM.
pub struct WarmupData {
    pub context: String,
}

/// Spawn background tasks to fetch accounts, holdings, and performance,
/// then format them into a context string the LLM can consume.
pub fn spawn_warmup(client: GhostfolioClient) -> oneshot::Receiver<WarmupData> {
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let result = fetch_context(&client).await;
        let _ = tx.send(result);
    });

    rx
}

async fn fetch_context(client: &GhostfolioClient) -> WarmupData {
    // Fetch accounts, holdings, and performance in parallel
    let (accounts, holdings, performance) = tokio::join!(
        client.get("/api/v1/account"),
        client.get("/api/v1/portfolio/holdings"),
        client.get_with_query("/api/v2/portfolio/performance", &[("range", "max")]),
    );

    let mut sections = Vec::new();

    match accounts {
        Ok(data) => {
            info!("warmup: accounts loaded");
            sections.push(format!("## Accounts\n```json\n{}\n```", truncate_json(&data)));
        }
        Err(e) => warn!(error = %e, "warmup: failed to fetch accounts"),
    }

    match holdings {
        Ok(data) => {
            info!("warmup: holdings loaded");
            sections.push(format!("## Holdings\n```json\n{}\n```", truncate_json(&data)));
        }
        Err(e) => warn!(error = %e, "warmup: failed to fetch holdings"),
    }

    match performance {
        Ok(data) => {
            info!("warmup: performance loaded");
            sections.push(format!("## Performance\n```json\n{}\n```", truncate_json(&data)));
        }
        Err(e) => warn!(error = %e, "warmup: failed to fetch performance"),
    }

    let context = if sections.is_empty() {
        String::new()
    } else {
        format!(
            "Here is the user's current portfolio data (pre-loaded for context — do not repeat it back unless asked):\n\n{}",
            sections.join("\n\n")
        )
    };

    info!(len = context.len(), "warmup: context ready");
    WarmupData { context }
}

/// Truncate JSON to avoid blowing up the context window.
fn truncate_json(value: &serde_json::Value) -> String {
    let s = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    if s.len() > 8000 {
        format!("{}... (truncated)", &s[..8000])
    } else {
        s
    }
}
