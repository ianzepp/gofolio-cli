//! Fire-and-forget LangSmith trace posting.
//!
//! Reads config from the same env vars as the server:
//!   LANGCHAIN_TRACING_V2, LANGCHAIN_ENDPOINT, LANGCHAIN_API_KEY, LANGCHAIN_PROJECT

use std::time::Duration;

use chrono::Utc;
use serde::Serialize;
use tracing::{info, warn};

use crate::config::Config;

/// LangSmith configuration, resolved from config file + environment.
#[derive(Clone)]
pub struct LangSmithConfig {
    pub endpoint: String,
    pub api_key: String,
    pub project: String,
}

impl LangSmithConfig {
    /// Load from config (env > config file). Returns None if key is missing.
    pub fn from_config(config: &Config) -> Option<Self> {
        let api_key = config.langchain_api_key()?;
        let endpoint = std::env::var("LANGCHAIN_ENDPOINT")
            .unwrap_or_else(|_| "https://api.smith.langchain.com".to_string());
        let project = config.langchain_project();

        info!(project = %project, "langsmith: tracing enabled");
        Some(Self { endpoint, api_key, project })
    }
}

/// A trace handle representing a parent run. Use this to add child runs
/// and finalize the trace.
#[derive(Clone)]
pub struct Trace {
    config: LangSmithConfig,
    http: reqwest::Client,
    pub run_id: String,
}

impl Trace {
    /// Create and POST a new parent "chain" run. Returns a handle for child runs.
    pub fn start(config: &LangSmithConfig, model: &str, user_input: &str) -> Self {
        let run_id = uuid();
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        let body = CreateRun {
            id: &run_id,
            name: "cli-agent-chat",
            run_type: "chain",
            session_name: &config.project,
            inputs: &serde_json::json!({
                "question": user_input,
                "model": model,
                "source": "ghostfolio-cli",
            }),
            start_time: &now(),
            parent_run_id: None,
            extra: None,
        };

        let trace = Self {
            config: config.clone(),
            http: http.clone(),
            run_id: run_id.clone(),
        };

        post_run(http, config.clone(), body);
        trace
    }

    /// Post a child "llm" run for an individual Anthropic API call.
    pub fn log_llm_call(
        &self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        duration_ms: u64,
        stop_reason: &str,
    ) {
        let child_id = uuid();
        let start = Utc::now() - chrono::Duration::milliseconds(duration_ms as i64);

        let body = CreateRun {
            id: &child_id,
            name: model,
            run_type: "llm",
            session_name: &self.config.project,
            inputs: &serde_json::json!({
                "model": model,
            }),
            start_time: &start.to_rfc3339(),
            parent_run_id: Some(&self.run_id),
            extra: Some(&serde_json::json!({
                "metadata": {
                    "ls_model_name": model,
                    "ls_provider": "anthropic",
                    "ls_model_type": "chat",
                },
            })),
        };

        let patch = PatchRun {
            outputs: &serde_json::json!({
                "stop_reason": stop_reason,
            }),
            end_time: &now(),
            extra: Some(&serde_json::json!({
                "metadata": {
                    "ls_model_name": model,
                    "ls_provider": "anthropic",
                    "ls_model_type": "chat",
                    "token_usage": {
                        "input_tokens": input_tokens,
                        "output_tokens": output_tokens,
                        "total_tokens": input_tokens + output_tokens,
                    },
                },
            })),
        };

        post_run(self.http.clone(), self.config.clone(), body);
        patch_run(self.http.clone(), self.config.clone(), &child_id, patch);
    }

    /// Post a child "tool" run for a tool invocation.
    pub fn log_tool_call(
        &self,
        tool_name: &str,
        duration_ms: u64,
        success: bool,
    ) {
        let child_id = uuid();
        let start = Utc::now() - chrono::Duration::milliseconds(duration_ms as i64);

        let body = CreateRun {
            id: &child_id,
            name: tool_name,
            run_type: "tool",
            session_name: &self.config.project,
            inputs: &serde_json::json!({ "tool": tool_name }),
            start_time: &start.to_rfc3339(),
            parent_run_id: Some(&self.run_id),
            extra: None,
        };

        let patch = PatchRun {
            outputs: &serde_json::json!({ "success": success }),
            end_time: &now(),
            extra: None,
        };

        post_run(self.http.clone(), self.config.clone(), body);
        patch_run(self.http.clone(), self.config.clone(), &child_id, patch);
    }

    /// Finalize the parent run with outputs.
    pub fn finish(
        &self,
        response_text: &str,
        total_input_tokens: u64,
        total_output_tokens: u64,
        steps: usize,
    ) {
        let patch = PatchRun {
            outputs: &serde_json::json!({
                "response": truncate(response_text, 2000),
                "total_input_tokens": total_input_tokens,
                "total_output_tokens": total_output_tokens,
                "steps": steps,
            }),
            end_time: &now(),
            extra: None,
        };

        patch_run(self.http.clone(), self.config.clone(), &self.run_id, patch);
    }

    /// Finalize the parent run with an error.
    pub fn finish_error(&self, error: &str) {
        let patch = PatchRun {
            outputs: &serde_json::json!({ "error": error }),
            end_time: &now(),
            extra: None,
        };

        patch_run(self.http.clone(), self.config.clone(), &self.run_id, patch);
    }
}

#[derive(Serialize)]
struct CreateRun<'a> {
    id: &'a str,
    name: &'a str,
    run_type: &'a str,
    session_name: &'a str,
    inputs: &'a serde_json::Value,
    start_time: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_run_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<&'a serde_json::Value>,
}

#[derive(Serialize)]
struct PatchRun<'a> {
    outputs: &'a serde_json::Value,
    end_time: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra: Option<&'a serde_json::Value>,
}

/// Fire-and-forget POST /runs.
fn post_run(http: reqwest::Client, config: LangSmithConfig, body: CreateRun<'_>) {
    let url = format!("{}/runs", config.endpoint);
    let api_key = config.api_key.clone();

    // Serialize before spawning to avoid lifetime issues
    let json = serde_json::to_vec(&body).unwrap_or_default();

    tokio::spawn(async move {
        let result = http
            .post(&url)
            .header("x-api-key", &api_key)
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await;

        match result {
            Ok(r) if !r.status().is_success() => {
                warn!(status = %r.status(), "langsmith: POST /runs failed");
            }
            Err(e) => warn!(error = %e, "langsmith: POST /runs error"),
            _ => {}
        }
    });
}

/// Fire-and-forget PATCH /runs/{run_id}.
fn patch_run(http: reqwest::Client, config: LangSmithConfig, run_id: &str, body: PatchRun<'_>) {
    let url = format!("{}/runs/{}", config.endpoint, run_id);
    let api_key = config.api_key.clone();

    let json = serde_json::to_vec(&body).unwrap_or_default();

    tokio::spawn(async move {
        let result = http
            .patch(&url)
            .header("x-api-key", &api_key)
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await;

        match result {
            Ok(r) if !r.status().is_success() => {
                warn!(status = %r.status(), "langsmith: PATCH /runs failed");
            }
            Err(e) => warn!(error = %e, "langsmith: PATCH /runs error"),
            _ => {}
        }
    });
}

fn uuid() -> String {
    // Simple v4 UUID from random bytes
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).unwrap_or(());
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 1
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u16::from_be_bytes([bytes[4], bytes[5]]),
        u16::from_be_bytes([bytes[6], bytes[7]]),
        u16::from_be_bytes([bytes[8], bytes[9]]),
        // last 6 bytes as a single hex number
        u64::from_be_bytes([0, 0, bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]]),
    )
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() > max { &s[..max] } else { s }
}
