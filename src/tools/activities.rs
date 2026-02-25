use crate::api::{ApiError, GhostfolioClient};

pub async fn list_activities(client: &GhostfolioClient) -> Result<serde_json::Value, ApiError> {
    client.get("/api/v1/order").await
}
