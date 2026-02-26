pub mod client;
pub mod tools;
pub mod types;

use std::time::Instant;

use tokio::sync::mpsc;
use tracing::{info, warn};

use self::client::AnthropicClient;
use self::types::{AgentError, Content, ContentBlock, Message, ToolCallRecord};
use crate::api::GhostfolioClient;
use crate::langsmith::{LangSmithConfig, Trace};
use crate::text::truncate_utf8;
use crate::tools as tool_dispatch;

const MAX_TOOL_ROUNDS: usize = 20;
const MAX_TOKENS: u32 = 4096;
const SYSTEM_PROMPT: &str = include_str!("system.md");

/// Event sent from the agent task to the TUI.
#[derive(Debug)]
pub enum AgentEvent {
    ToolCall(ToolCallRecord),
    ChartData(serde_json::Value),
    Response {
        text: String,
        input_tokens: u64,
        output_tokens: u64,
        last_input_tokens: u64,
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
    langsmith: Option<LangSmithConfig>,
    tx: mpsc::UnboundedSender<AgentEvent>,
) {
    tokio::spawn(async move {
        match run_loop(
            &api_client,
            &anthropic_key,
            &model,
            history,
            langsmith.as_ref(),
            &tx,
        )
        .await
        {
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
    langsmith: Option<&LangSmithConfig>,
    tx: &mpsc::UnboundedSender<AgentEvent>,
) -> Result<(), AgentError> {
    let client = AnthropicClient::new(anthropic_key.to_string())?;
    let tools = tools::all_tools();

    // Extract user input for the trace
    let user_input = extract_last_user_input(&messages);

    // Start LangSmith trace if configured
    let trace = langsmith.map(|cfg| Trace::start(cfg, model, &user_input));

    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;
    let mut last_input_tokens: u64 = 0;

    for (steps, round) in (0..MAX_TOOL_ROUNDS).enumerate() {
        info!(
            round = round + 1,
            messages = messages.len(),
            "agent: llm round start"
        );

        let llm_start = Instant::now();
        let response = client
            .chat(model, MAX_TOKENS, SYSTEM_PROMPT, &messages, Some(&tools))
            .await?;
        let llm_duration_ms = llm_start.elapsed().as_millis() as u64;

        total_input_tokens += response.input_tokens;
        total_output_tokens += response.output_tokens;
        last_input_tokens = response.input_tokens;

        // Log LLM call to LangSmith
        if let Some(ref t) = trace {
            t.log_llm_call(
                model,
                response.input_tokens,
                response.output_tokens,
                llm_duration_ms,
                &response.stop_reason,
            );
        }

        info!(
            round = round + 1,
            stop_reason = %response.stop_reason,
            blocks = response.content.len(),
            "agent: llm round result"
        );

        if response.stop_reason == "end_turn" || response.stop_reason == "max_tokens" {
            let text = extract_text(&response.content);

            if let Some(ref t) = trace {
                t.finish(&text, total_input_tokens, total_output_tokens, steps + 1);
            }

            let _ = tx.send(AgentEvent::Response {
                text,
                input_tokens: total_input_tokens,
                output_tokens: total_output_tokens,
                last_input_tokens,
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
                        // Emit chart data for rendering in the TUI
                        if data.get("chart_type").is_some() {
                            let _ = tx.send(AgentEvent::ChartData(data.clone()));
                        }

                        // Truncate large responses to avoid context bloat
                        let s = data.to_string();
                        if s.len() > 4000 {
                            (format!("{}... (truncated)", truncate_utf8(&s, 4000)), false)
                        } else {
                            (s, false)
                        }
                    }
                    Err(e) => (format!("error: {e}"), true),
                };

                // Log tool call to LangSmith
                if let Some(ref t) = trace {
                    t.log_tool_call(name, duration_ms, !is_error);
                }

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

            // Prune tool results from earlier rounds to save context
            // Keep only the latest tool result message (the one we just pushed) intact
            let last_idx = messages.len() - 1;
            for (idx, msg) in messages.iter_mut().enumerate() {
                if idx >= last_idx {
                    break;
                }
                if let Content::Blocks(ref mut blocks) = msg.content {
                    for block in blocks.iter_mut() {
                        if let ContentBlock::ToolResult { content, .. } = block {
                            if content.len() > 100 {
                                *content = format!("{}...", truncate_utf8(content, 100));
                            }
                        }
                    }
                }
            }

            continue;
        }

        warn!(stop_reason = %response.stop_reason, "agent: unknown stop_reason");

        if let Some(ref t) = trace {
            t.finish_error(&format!("Unexpected stop reason: {}", response.stop_reason));
        }

        let _ = tx.send(AgentEvent::Error(format!(
            "Unexpected stop reason: {}",
            response.stop_reason
        )));
        return Ok(());
    }

    if let Some(ref t) = trace {
        t.finish_error(&format!("Max rounds exceeded: {}", MAX_TOOL_ROUNDS));
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

fn extract_last_user_input(messages: &[Message]) -> String {
    messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| match &m.content {
            Content::Text(t) => t.clone(),
            Content::Blocks(_) => "(tool results)".to_string(),
        })
        .unwrap_or_default()
}
