use crate::api::{ApiError, GhostfolioClient};

use super::query_params;

pub async fn list_activities(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let params = query_params(
        input,
        &[
            "range",
            "accounts",
            "assetClasses",
            "dataSource",
            "symbol",
            "tags",
            "sortColumn",
            "sortDirection",
            "skip",
            "take",
        ],
    );
    let refs: Vec<(&str, &str)> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    client.get_with_query("/api/v1/order", &refs).await
}
