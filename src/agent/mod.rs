pub mod client;
pub mod tools;
pub mod types;

use std::time::Instant;

use tokio::sync::mpsc;
use tracing::{info, warn};

use self::client::AnthropicClient;
use self::types::{AgentError, Content, ContentBlock, Message, ToolCallRecord};
use crate::api::GhostfolioClient;
use crate::tools as tool_dispatch;

const MAX_TOOL_ROUNDS: usize = 20;
const MAX_TOKENS: u32 = 4096;

const SYSTEM_PROMPT: &str = r#"You are a financial portfolio assistant for Ghostfolio, a wealth management platform. You help users understand their investment portfolio, analyze performance, research assets, and answer financial questions.

When the user asks about their portfolio, use the available tools to fetch real data before responding. Present data in clear, formatted tables and summaries. Use markdown formatting for readability.

Key guidelines:
- Always fetch fresh data rather than guessing or using stale information
- Present monetary values with proper formatting (currency symbols, commas, decimal places)
- Calculate percentages and changes when comparing values
- If a tool returns an error, explain the issue clearly and suggest alternatives
- For asset research, use search_assets first to find the correct symbol and data source
- Present holdings and performance data in tables when appropriate
- Be concise but thorough — include key metrics without overwhelming detail"#;

/// Event sent from the agent task to the TUI.
#[derive(Debug)]
pub enum AgentEvent {
    ToolCall(ToolCallRecord),
    Response {
        text: String,
        input_tokens: u64,
        output_tokens: u64,
        steps: usize,
    },
    Error(String),
}

/// Run the agent ReAct loop in a background task.
pub fn spawn_agent(
    api_client: GhostfolioClient,
    anthropic_key: String,
    model: String,
    history: Vec<Message>,
    tx: mpsc::UnboundedSender<AgentEvent>,
) {
    tokio::spawn(async move {
        match run_loop(&api_client, &anthropic_key, &model, history, &tx).await {
            Ok(()) => {}
            Err(e) => {
                let _ = tx.send(AgentEvent::Error(e.to_string()));
            }
        }
    });
}

async fn run_loop(
    api_client: &GhostfolioClient,
    anthropic_key: &str,
    model: &str,
    mut messages: Vec<Message>,
    tx: &mpsc::UnboundedSender<AgentEvent>,
) -> Result<(), AgentError> {
    let client = AnthropicClient::new(anthropic_key.to_string())?;
    let tools = tools::all_tools();

    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;

    for (steps, round) in (0..MAX_TOOL_ROUNDS).enumerate() {
        info!(round = round + 1, messages = messages.len(), "agent: llm round start");

        let response = client
            .chat(model, MAX_TOKENS, SYSTEM_PROMPT, &messages, Some(&tools))
            .await?;

        total_input_tokens += response.input_tokens;
        total_output_tokens += response.output_tokens;

        info!(
            round = round + 1,
            stop_reason = %response.stop_reason,
            blocks = response.content.len(),
            "agent: llm round result"
        );

        if response.stop_reason == "end_turn" || response.stop_reason == "max_tokens" {
            let text = extract_text(&response.content);
            let _ = tx.send(AgentEvent::Response {
                text,
                input_tokens: total_input_tokens,
                output_tokens: total_output_tokens,
                steps: steps + 1,
            });
            return Ok(());
        }

        if response.stop_reason == "tool_use" {
            // Append assistant message with tool_use blocks
            messages.push(Message {
                role: "assistant".to_string(),
                content: Content::Blocks(response.content.clone()),
            });

            // Dispatch each tool and collect results
            let mut tool_results = Vec::new();
            for block in &response.content {
                let ContentBlock::ToolUse { id, name, input } = block else {
                    continue;
                };

                let start = Instant::now();
                let result = tool_dispatch::dispatch(api_client, name, input).await;
                let duration_ms = start.elapsed().as_millis() as u64;

                let (content, is_error) = match result {
                    Ok(data) => {
                        // Truncate large responses to avoid context bloat
                        let s = data.to_string();
                        if s.len() > 4000 {
                            (format!("{}... (truncated)", &s[..4000]), false)
                        } else {
                            (s, false)
                        }
                    }
                    Err(e) => (format!("error: {e}"), true),
                };

                let _ = tx.send(AgentEvent::ToolCall(ToolCallRecord {
                    name: name.clone(),
                    duration_ms,
                    success: !is_error,
                }));

                tool_results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content,
                    is_error: Some(is_error),
                });
            }

            // Append tool results as user message
            messages.push(Message {
                role: "user".to_string(),
                content: Content::Blocks(tool_results),
            });

            continue;
        }

        warn!(stop_reason = %response.stop_reason, "agent: unknown stop_reason");
        let _ = tx.send(AgentEvent::Error(format!(
            "Unexpected stop reason: {}",
            response.stop_reason
        )));
        return Ok(());
    }

    Err(AgentError::MaxRounds(MAX_TOOL_ROUNDS))
}

fn extract_text(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}
