use crate::api::{ApiError, GhostfolioClient};

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

pub async fn get_market_data(client: &GhostfolioClient) -> Result<serde_json::Value, ApiError> {
    client.get("/api/v1/market-data/markets").await
}
