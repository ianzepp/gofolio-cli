use crate::api::{ApiError, GhostfolioClient};

pub async fn get_performance(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let range = input["range"].as_str().unwrap_or("max");
    client
        .get_with_query("/api/v1/portfolio/performance", &[("range", range)])
        .await
}

pub async fn get_dividends(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let range = input["range"].as_str().unwrap_or("max");
    client
        .get_with_query("/api/v1/portfolio/dividends", &[("range", range)])
        .await
}

pub async fn get_investments(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let range = input["range"].as_str().unwrap_or("max");
    client
        .get_with_query("/api/v1/portfolio/investments", &[("range", range)])
        .await
}
