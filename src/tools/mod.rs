mod accounts;
mod activities;
mod assets;
mod benchmarks;
pub mod calculator;
pub mod charts;
mod performance;
mod portfolio;

use crate::api::{ApiError, GhostfolioClient};

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
