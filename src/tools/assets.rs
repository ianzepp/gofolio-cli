use crate::api::{ApiError, GhostfolioClient};

pub async fn search_assets(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let query = input["query"]
        .as_str()
        .ok_or_else(|| ApiError::Request("missing query".to_string()))?;
    client
        .get_with_query("/api/v1/symbol/lookup", &[("query", query)])
        .await
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
    client
        .get(&format!("/api/v1/asset/{data_source}/{symbol}"))
        .await
}

pub async fn get_market_data(client: &GhostfolioClient) -> Result<serde_json::Value, ApiError> {
    client.get("/api/v1/market-data/markets").await
}
