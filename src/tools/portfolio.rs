use crate::api::{ApiError, GhostfolioClient};

pub async fn get_portfolio_summary(client: &GhostfolioClient) -> Result<serde_json::Value, ApiError> {
    client.get("/api/v1/portfolio/details").await
}

pub async fn get_holdings(client: &GhostfolioClient) -> Result<serde_json::Value, ApiError> {
    client.get("/api/v1/portfolio/holdings").await
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
