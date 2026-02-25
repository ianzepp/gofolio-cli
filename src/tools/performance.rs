use crate::api::{ApiError, GhostfolioClient};

use super::query_params;

pub async fn get_performance(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let params = query_params(input, &["range", "accounts", "assetClasses", "dataSource", "symbol", "tags"]);
    let refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    client.get_with_query("/api/v1/portfolio/performance", &refs).await
}

pub async fn get_dividends(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let params = query_params(input, &["range", "groupBy", "accounts", "assetClasses", "dataSource", "symbol", "tags"]);
    let refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    client.get_with_query("/api/v1/portfolio/dividends", &refs).await
}

pub async fn get_investments(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let params = query_params(input, &["range", "groupBy", "accounts", "assetClasses", "dataSource", "symbol", "tags"]);
    let refs: Vec<(&str, &str)> = params.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    client.get_with_query("/api/v1/portfolio/investments", &refs).await
}
