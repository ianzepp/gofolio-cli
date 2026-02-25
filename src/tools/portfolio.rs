use crate::api::{ApiError, GhostfolioClient};

use super::query_params;

pub async fn get_portfolio_summary(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let params = query_params(input, &["range", "accounts", "assetClasses", "dataSource", "symbol", "tags"]);
    let refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    client.get_with_query("/api/v1/portfolio/details", &refs).await
}

pub async fn get_holdings(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let params = query_params(input, &["query", "holdingType", "range", "accounts", "assetClasses", "dataSource", "symbol", "tags"]);
    let refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    client.get_with_query("/api/v1/portfolio/holdings", &refs).await
}

pub async fn get_holding_detail(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let data_source = input["dataSource"]
        .as_str()
        .ok_or_else(|| ApiError::Request("missing dataSource".to_string()))?;
    let symbol = input["symbol"]
        .as_str()
        .ok_or_else(|| ApiError::Request("missing symbol".to_string()))?;
    client
        .get(&format!("/api/v1/portfolio/holding/{data_source}/{symbol}"))
        .await
}
