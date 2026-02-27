use crate::api::{ApiError, GhostfolioClient};
use chrono::Utc;

use super::{encode_path_segment, query_params};

pub async fn search_assets(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let params = query_params(input, &["query", "includeIndices"]);
    let refs: Vec<(&str, &str)> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    client.get_with_query("/api/v1/symbol/lookup", &refs).await
}

pub async fn get_asset_profile(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let data_source = input["dataSource"]
        .as_str()
        .ok_or_else(|| ApiError::Request("missing dataSource".to_string()))?;
    let symbol = input["symbol"]
        .as_str()
        .ok_or_else(|| ApiError::Request("missing symbol".to_string()))?;
    let data_source = encode_path_segment(data_source);
    let symbol = encode_path_segment(symbol);
    client
        .get(&format!("/api/v1/asset/{data_source}/{symbol}"))
        .await
}

pub async fn get_fear_greed_index(client: &GhostfolioClient) -> Result<serde_json::Value, ApiError> {
    client.get("/api/v1/market-data/markets").await
}

pub async fn exchange_rate(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let from_currency = input["fromCurrency"]
        .as_str()
        .ok_or_else(|| ApiError::Request("missing fromCurrency".to_string()))?
        .to_uppercase();
    let to_currency = input["toCurrency"]
        .as_str()
        .ok_or_else(|| ApiError::Request("missing toCurrency".to_string()))?
        .to_uppercase();
    let date_string = input
        .get("date")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());

    let symbol = format!("{from_currency}-{to_currency}");
    let symbol = encode_path_segment(&symbol);
    let date_string = encode_path_segment(&date_string);

    client
        .get(&format!("/api/v1/exchange-rate/{symbol}/{date_string}"))
        .await
}

pub async fn price_history(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let data_source = input["dataSource"]
        .as_str()
        .ok_or_else(|| ApiError::Request("missing dataSource".to_string()))?;
    let symbol = input["symbol"]
        .as_str()
        .ok_or_else(|| ApiError::Request("missing symbol".to_string()))?;
    let days = input
        .get("days")
        .and_then(|v| v.as_i64())
        .filter(|d| *d > 0)
        .unwrap_or(30);

    let data_source = encode_path_segment(data_source);
    let symbol = encode_path_segment(symbol);
    let days_s = days.to_string();
    let query = [("includeHistoricalData", days_s.as_str())];

    client
        .get_with_query(&format!("/api/v1/symbol/{data_source}/{symbol}"), &query)
        .await
}
