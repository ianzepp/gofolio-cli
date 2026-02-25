use crate::api::{ApiError, GhostfolioClient};

pub async fn list_accounts(client: &GhostfolioClient) -> Result<serde_json::Value, ApiError> {
    client.get("/api/v1/account").await
}

pub async fn get_account_balances(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let id = input["id"]
        .as_str()
        .ok_or_else(|| ApiError::Request("missing id".to_string()))?;
    client.get(&format!("/api/v1/account/{id}/balances")).await
}
