use crate::api::{ApiError, GhostfolioClient};

pub async fn get_benchmarks(client: &GhostfolioClient) -> Result<serde_json::Value, ApiError> {
    client.get("/api/v1/benchmarks").await
}
