use serde::Deserialize;

use crate::config::Config;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("no access token configured")]
    NoAccessToken,
    #[error("auth request failed: {0}")]
    Request(String),
    #[error("auth failed: HTTP {status}: {body}")]
    Response { status: u16, body: String },
    #[error("auth response parse failed: {0}")]
    Parse(String),
}

#[derive(Deserialize)]
struct AuthResponse {
    #[serde(rename = "authToken")]
    auth_token: String,
}

/// Exchange an access token for a JWT via the Ghostfolio anonymous auth endpoint.
pub async fn exchange_token(
    http: &reqwest::Client,
    base_url: &str,
    access_token: &str,
) -> Result<String, AuthError> {
    let url = format!("{base_url}/api/v1/auth/anonymous");
    let body = serde_json::json!({ "accessToken": access_token });

    let response = http
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AuthError::Request(e.to_string()))?;

    let status = response.status().as_u16();
    if status != 200 && status != 201 {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "no body".to_string());
        return Err(AuthError::Response { status, body });
    }

    let auth: AuthResponse = response
        .json()
        .await
        .map_err(|e| AuthError::Parse(e.to_string()))?;

    Ok(auth.auth_token)
}

/// Authenticate using config/env settings. Returns (jwt, base_url).
pub async fn authenticate(config: &Config) -> Result<(String, String), AuthError> {
    let base_url = config.ghostfolio_url();
    let access_token = config.access_token().ok_or(AuthError::NoAccessToken)?;

    // If it looks like a JWT already (3 dot-separated parts), use directly
    if access_token.split('.').count() == 3 {
        return Ok((access_token, base_url));
    }

    let http = reqwest::Client::new();
    let jwt = exchange_token(&http, &base_url, &access_token).await?;
    Ok((jwt, base_url))
}
