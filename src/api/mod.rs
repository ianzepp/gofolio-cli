pub mod auth;

use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Request(String),
    #[error("HTTP {status}: {body}")]
    Response { status: u16, body: String },
    #[error("response parse failed: {0}")]
    Parse(String),
}

/// HTTP client for the Ghostfolio REST API.
#[derive(Clone)]
pub struct GhostfolioClient {
    http: reqwest::Client,
    base_url: String,
    jwt: String,
}

impl GhostfolioClient {
    pub fn new(base_url: String, jwt: String) -> Self {
        let http = build_http_client();

        Self {
            http,
            base_url,
            jwt,
        }
    }

    /// GET request to a Ghostfolio API endpoint. Returns raw JSON.
    pub async fn get(&self, path: &str) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}{path}", self.base_url);
        let response = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.jwt))
            .send()
            .await
            .map_err(|e| ApiError::Request(e.to_string()))?;

        let status = response.status().as_u16();
        if status != 200 {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "no body".to_string());
            return Err(ApiError::Response { status, body });
        }

        response
            .json()
            .await
            .map_err(|e| ApiError::Parse(e.to_string()))
    }

    /// GET request with query parameters.
    pub async fn get_with_query(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value, ApiError> {
        let url = format!("{}{path}", self.base_url);
        let response = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.jwt))
            .query(query)
            .send()
            .await
            .map_err(|e| ApiError::Request(e.to_string()))?;

        let status = response.status().as_u16();
        if status != 200 {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "no body".to_string());
            return Err(ApiError::Response { status, body });
        }

        response
            .json()
            .await
            .map_err(|e| ApiError::Parse(e.to_string()))
    }
}

fn build_http_client() -> reqwest::Client {
    match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!(
                "Warning: failed to build configured HTTP client, falling back to default: {e}"
            );
            reqwest::Client::new()
        }
    }
}
