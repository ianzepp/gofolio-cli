# Ghostfolio CLI — Rust Implementation Plan

## Context

The current Ghostfolio agent architecture embeds an LLM inside the Ghostfolio API server, requiring invasive patches to the upstream codebase. This is unmaintainable and blocks independent distribution. The solution: move the LLM to a standalone Rust CLI that uses Ghostfolio's existing 100+ HTTP API endpoints as tools. Users point the CLI at any vanilla Ghostfolio instance — no patches required.

The CLI replicates the exact Bloomberg-terminal UI/UX of the current Ink-based `gauntlet/cli/` (two-column layout, amber theme, markdown chat, modal overlays), but built with ratatui in Rust. It lives at `cli/` in this repo for grading visibility, with compiled binaries published to a separate public repo for distribution.

## Reference Projects

- **prior** (`~/github/ianzepp/prior/kernel/src/llm/`): Anthropic client, ContentBlock types, ReAct loop with MAX_TOOL_ROUNDS=20
- **abbot** (`~/github/ianzepp/abbot/`): Tool spec JSON files, clap CLI structure, retry policies
- **gauntlet/cli/**: Current Ink TUI — exact UI spec to replicate (theme.ts, app.tsx, chat-panel.tsx, etc.)

## Project Structure

```
cli/                              # New directory at repo root
├── Cargo.toml
├── src/
│   ├── main.rs                   # Clap entry point + subcommands
│   ├── app.rs                    # App state machine + event loop (tokio select!)
│   ├── config.rs                 # ~/.config/ghostfolio-cli/config.toml
│   ├── theme.rs                  # Color constants matching gauntlet/cli/src/theme.ts
│   ├── markdown.rs               # Markdown → ratatui Spans (headings, tables, code, bold, bullets)
│   ├── api/
│   │   ├── mod.rs                # Ghostfolio HTTP client (reqwest + JWT auth)
│   │   └── auth.rs               # Token exchange, JWT management
│   ├── agent/
│   │   ├── mod.rs                # ReAct loop (adapted from prior/kernel/src/room/message.rs)
│   │   ├── client.rs             # Anthropic Messages API (from prior/kernel/src/llm/client.rs)
│   │   ├── types.rs              # ContentBlock, Message, Tool, ChatResponse (from prior/kernel/src/llm/types.rs)
│   │   └── tools.rs              # Tool definitions (JSON schemas for each Ghostfolio endpoint)
│   ├── tools/                    # Tool implementations — each maps to Ghostfolio API calls
│   │   ├── mod.rs                # Tool dispatch: name → handler function
│   │   ├── portfolio.rs          # get_portfolio_summary, get_holdings, get_holding_detail
│   │   ├── performance.rs        # get_performance, get_dividends, get_investments
│   │   ├── activities.rs         # list_activities, get_activity
│   │   ├── accounts.rs           # list_accounts, get_account, get_account_balances
│   │   ├── assets.rs             # search_assets, get_asset_profile, get_market_data
│   │   └── benchmarks.rs         # get_benchmarks, get_benchmark_performance
│   └── ui/
│       ├── mod.rs                # Top-level render function
│       ├── layout.rs             # Two-column constraint split
│       ├── chat.rs               # Scrollable chat panel with markdown rendering
│       ├── sidebar.rs            # Model panel, Tools panel, Session panel (width=36)
│       ├── input.rs              # Input bar with ">>> " prompt
│       ├── status.rs             # Top status bar with keyboard shortcuts
│       ├── modal.rs              # Modal overlay system (double-border)
│       └── login.rs              # Login screen (server URL + access token)
```

## Dependencies (Cargo.toml)

```toml
[package]
name = "ghostfolio-cli"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
ratatui = "0.29"
crossterm = "0.28"
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
dirs = "6"
toml = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

## Implementation Phases

### Phase 1: Project Scaffold + Config + Auth

1. Create `cli/Cargo.toml` with dependencies
2. `main.rs`: Clap CLI with subcommands: `chat` (default, TUI mode), `config` (manage settings)
3. `config.rs`: Load/save `~/.config/ghostfolio-cli/config.toml` (ghostfolio_url, access_token, anthropic_api_key, model)
4. `api/auth.rs`: Token exchange via `POST /api/v1/auth/anonymous` (adapted from gauntlet/cli/src/api.ts:53-67)
5. `api/mod.rs`: GhostfolioClient struct with reqwest + JWT, base URL resolution from config/env

### Phase 2: Anthropic Client + ReAct Loop

1. `agent/types.rs`: Port ContentBlock, Message, Content, Tool, ChatResponse from `prior/kernel/src/llm/types.rs` — remove Frame/Data dependencies, keep serde derives
2. `agent/client.rs`: Port AnthropicClient from `prior/kernel/src/llm/client.rs` — direct reqwest to Anthropic API
3. `agent/tools.rs`: Define ~12 tool schemas as `Tool` structs with JSON input_schema (one per Ghostfolio operation)
4. `agent/mod.rs`: ReAct loop adapted from `prior/kernel/src/room/message.rs:run_actor_loop()` — simplified since no Frame/Caller/pipe layer needed. Direct: call Anthropic → check stop_reason → dispatch tool → loop

### Phase 3: Tool Implementations

Each tool is a function: `async fn(client: &GhostfolioClient, input: Value) -> Result<String, ToolError>`

1. `tools/portfolio.rs`:
   - `get_portfolio_summary` → `GET /api/v1/portfolio/details`
   - `get_holdings` → `GET /api/v1/portfolio/holdings`
   - `get_holding_detail` → `GET /api/v1/portfolio/holding/:dataSource/:symbol`
2. `tools/performance.rs`:
   - `get_performance` → `GET /api/v1/portfolio/performance`
   - `get_dividends` → `GET /api/v1/portfolio/dividends`
   - `get_investments` → `GET /api/v1/portfolio/investments`
3. `tools/activities.rs`:
   - `list_activities` → `GET /api/v1/order`
4. `tools/accounts.rs`:
   - `list_accounts` → `GET /api/v1/account`
   - `get_account_balances` → `GET /api/v1/account/:id/balances`
5. `tools/assets.rs`:
   - `search_assets` → `GET /api/v1/symbol/lookup`
   - `get_asset_profile` → `GET /api/v1/asset/:dataSource/:symbol`
   - `get_market_data` → `GET /api/v1/market-data/markets`
6. `tools/benchmarks.rs`:
   - `get_benchmarks` → `GET /api/v1/benchmarks`
7. `tools/mod.rs`: Dispatch function mapping tool name string → handler

### Phase 4: TUI — Layout + Theme + Static Panels

1. `theme.rs`: Color constants (amber=#FF8800, green=#00FF00, red=#FF4444, warning=#FFAA00, muted=#888888, border=#444444)
2. `ui/layout.rs`: Two-column split — left=Min(20), right=Length(36). Vertical: StatusBar(1), Header(1), Content(fill), Input(2)
3. `ui/status.rs`: Render keyboard shortcut chips (Ctrl+N/Y/R/P/T/L/Q)
4. `ui/sidebar.rs`: Three stacked blocks — MODEL (name + traits), TOOLS (name + duration + checkmark), SESSION (turn/tokens/latency/steps/verified/feedback)
5. `ui/input.rs`: ">>> " prompt with text editing (crossterm key events)
6. `ui/login.rs`: Centered double-border dialog with URL + token fields

### Phase 5: TUI — Chat Panel + Markdown

1. `markdown.rs`: Block-level parser producing enum (Heading, Hr, Table, Code, Text) — port logic from `gauntlet/cli/src/components/chat-panel.tsx:parseBlocks()`
2. Inline parser: bold (**text**), bullets (- → bullet char)
3. Table renderer: column width calculation, │─┼─ borders
4. `ui/chat.rs`: Scrollable viewport with line estimation, role labels (YOU/AGT/SYS in amber), spinner when loading

### Phase 6: TUI — Event Loop + Modals + Integration

1. `app.rs`: Main event loop using `tokio::select!` — crossterm events + agent response channel
2. State machine: Init → Login → App (with modal substates)
3. `ui/modal.rs`: Double-border overlay with scrollable list + filter input (for model selection)
4. Connect agent ReAct loop to TUI: spawn agent task on message submit, stream tool calls to sidebar, display final response in chat
5. Keyboard handling: Ctrl+shortcuts when no modal, Esc to close modals, arrow keys in modals

### Phase 7: Polish + Release Pipeline

1. System prompt: Finance-focused system prompt for the agent
2. Conversation history: Persist across turns within a session
3. Error handling: Graceful display of API errors, auth failures, LLM errors
4. `.github/workflows/release-cli.yml`: Cross-compile (linux x86_64, macOS aarch64/x86_64), publish to `ianzepp/ghostfolio-cli` public repo
5. README in `cli/` with usage instructions

## Key Design Decisions

- **No streaming from Anthropic**: Use non-streaming Messages API (simpler, matches prior's pattern). The spinner shows "Thinking..." until complete response arrives. Tool calls update sidebar in real-time as each tool dispatches.
- **Tool result format**: Return raw JSON from Ghostfolio API responses. The LLM is good at interpreting structured data. Truncate responses >4000 chars to avoid context bloat.
- **Session = conversation history in memory**: No persistence layer initially. /new clears history. Can add SQLite later if needed.
- **Config precedence**: env vars > config file > defaults (matching current CLI pattern)

## Verification

1. `cargo build` compiles cleanly
2. `cargo clippy -- -D warnings` passes
3. Run `./target/debug/ghostfolio-cli config` — creates config file
4. Run `./target/debug/ghostfolio-cli` — shows login screen, enter Ghostfolio URL + access token
5. After auth, type "show me my portfolio" — agent calls get_portfolio_summary tool, displays result
6. Verify sidebar updates with tool calls (name + duration + checkmark)
7. Verify Ctrl+Q quits, Ctrl+N clears session, Ctrl+L returns to login
8. Verify markdown rendering: send a message that produces tables, headings, bold text
