mod accounts;
mod activities;
mod assets;
mod benchmarks;
pub mod calculator;
pub mod charts;
mod performance;
mod portfolio;

use std::collections::HashMap;
use std::path::Path;

use crate::api::{ApiError, GhostfolioClient};

#[derive(Clone)]
pub enum ToolDispatcher {
    Live(GhostfolioClient),
    Mock(MockFixtureSet),
}

impl ToolDispatcher {
    pub async fn dispatch(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        match self {
            Self::Live(client) => dispatch(client, tool_name, input).await,
            Self::Mock(fixtures) => fixtures.dispatch(tool_name, input),
        }
    }
}

#[derive(Clone, Default)]
pub struct MockFixtureSet {
    by_tool: HashMap<String, serde_json::Value>,
}

impl MockFixtureSet {
    pub fn load_dir(dir: &Path) -> Result<Self, ApiError> {
        let mut by_tool = HashMap::new();

        let entries = std::fs::read_dir(dir).map_err(|e| {
            ApiError::Request(format!(
                "failed to read fixture directory {}: {e}",
                dir.display()
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| ApiError::Request(e.to_string()))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if ext != "json" {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };

            let content = std::fs::read_to_string(&path).map_err(|e| {
                ApiError::Request(format!(
                    "failed to read fixture file {}: {e}",
                    path.display()
                ))
            })?;
            let value: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
                ApiError::Parse(format!("invalid json fixture {}: {e}", path.display()))
            })?;
            by_tool.insert(stem.to_string(), value);
        }

        Ok(Self { by_tool })
    }

    pub fn dispatch(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        let raw = self.by_tool.get(tool_name).ok_or_else(|| {
            ApiError::Request(format!(
                "mock fixture missing for tool '{tool_name}' (expected {tool_name}.json)"
            ))
        })?;

        if let Some(key) = extract_lookup_key(tool_name, input)
            && let Some(obj) = raw.as_object()
            && !obj.is_empty()
        {
            if let Some(value) = obj.get(&key) {
                return Ok(value.clone());
            }

            let key_lower = key.to_lowercase();
            if let Some((_, value)) = obj.iter().find(|(k, _)| k.to_lowercase() == key_lower) {
                return Ok(value.clone());
            }
        }

        Ok(raw.clone())
    }
}

pub async fn dispatch(
    client: &GhostfolioClient,
    tool_name: &str,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    match tool_name {
        "get_portfolio_summary" => portfolio::get_portfolio_summary(client, input).await,
        "get_holdings" => portfolio::get_holdings(client, input).await,
        "get_holding_detail" => portfolio::get_holding_detail(client, input).await,
        "get_performance" => performance::get_performance(client, input).await,
        "get_dividends" => performance::get_dividends(client, input).await,
        "get_investments" => performance::get_investments(client, input).await,
        "list_activities" => activities::list_activities(client, input).await,
        "list_accounts" => accounts::list_accounts(client).await,
        "get_account_balances" => accounts::get_account_balances(client, input).await,
        "search_assets" => assets::search_assets(client, input).await,
        "get_asset_profile" => assets::get_asset_profile(client, input).await,
        "get_market_data" => assets::get_market_data(client).await,
        "get_benchmarks" => benchmarks::get_benchmarks(client).await,
        "calculate" => calculator::evaluate(input).map_err(ApiError::Request),
        "chart_sparkline" => charts::sparkline(input).map_err(ApiError::Request),
        "chart_bar" => charts::bar(input).map_err(ApiError::Request),
        _ => Err(ApiError::Request(format!("unknown tool: {tool_name}"))),
    }
}

fn extract_lookup_key(tool_name: &str, input: &serde_json::Value) -> Option<String> {
    match tool_name {
        "get_holding_detail" | "get_asset_profile" => {
            let data_source = input.get("dataSource")?.as_str()?;
            let symbol = input.get("symbol")?.as_str()?;
            Some(format!("{data_source}:{symbol}"))
        }
        "get_account_balances" => input
            .get("id")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        "search_assets" => input
            .get("query")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        _ => None,
    }
}

/// Extract query parameters from a JSON input object for the given keys.
/// Only includes keys that are present and have string (or numeric) values.
fn query_params(input: &serde_json::Value, keys: &[&str]) -> Vec<(String, String)> {
    let mut params = Vec::new();
    for &key in keys {
        if let Some(val) = input.get(key) {
            if let Some(s) = val.as_str() {
                if !s.is_empty() {
                    params.push((key.to_string(), s.to_string()));
                }
            } else if let Some(n) = val.as_i64() {
                params.push((key.to_string(), n.to_string()));
            } else if let Some(n) = val.as_u64() {
                params.push((key.to_string(), n.to_string()));
            }
        }
    }
    params
}

/// Percent-encode a path segment to keep tool-driven API requests in-bounds.
fn encode_path_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for &b in segment.as_bytes() {
        let is_unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~');
        if is_unreserved {
            out.push(char::from(b));
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode_path_segment;

    #[test]
    fn encodes_reserved_and_unicode_bytes() {
        assert_eq!(encode_path_segment("../AAPL"), "..%2FAAPL");
        assert_eq!(encode_path_segment("BTC 🚀"), "BTC%20%F0%9F%9A%80");
    }
}
