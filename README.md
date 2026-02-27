# Ghostfolio CLI (Rust)

`cli/` is the standalone Rust terminal application for Ghostfolio chat + evals.

All `cargo` commands below are intended to be run from the `cli/` directory.

## What This CLI Includes

- Interactive Bloomberg-style chat UI (`ghostfolio` / `ghostfolio chat`)
- Tool-calling agent runtime (live API + fixture-backed eval mode)
- Config management (`ghostfolio config`)
- Eval runner with two display modes:
- TUI progress dashboard (default when stderr is a terminal)
- Plain console output (`--no-tui`)

## Prerequisites

- Rust toolchain (`cargo`, `rustc`)
- Ghostfolio API reachable (default `http://localhost:3333`)
- Ghostfolio access token
- At least one LLM key:
- `ANTHROPIC_API_KEY`
- `OPENROUTER_API_KEY`
- `OPENAI_API_KEY`

## Quick Start

From repo root:

```bash
cargo run --manifest-path cli/Cargo.toml
```

Or from `cli/`:

```bash
cargo run
```

Default command is `chat`.

## Commands

### `chat` (default)

```bash
cargo run -- chat
```

Starts the interactive terminal app:

- Login screen for Ghostfolio URL/token
- Chat pane with tool call timeline
- Session status bar with tokens, latency, and verification confidence
- Model picker (`Ctrl+P`)
- Inline charts (sparkline / bar) when chart tools are used

Chat keyboard shortcuts:

- `Ctrl+Q` or `Ctrl+C`: quit
- `Ctrl+N`: new session
- `Ctrl+Y`: thumbs up
- `Ctrl+R`: report/thumbs down
- `Ctrl+P`: model picker
- `Ctrl+L`: logout
- `PgUp` / `PgDn`: scroll
- `Shift+Up` / `Shift+Down`: line scroll
- `Home` / `End`: jump scroll

Slash commands:

- `/new` or `/clear`
- `/up`
- `/report`
- `/model`
- `/logout`
- `/quit` `/exit` `/q`
- `/help`

### `config`

Show current config:

```bash
cargo run -- config
```

Set config values:

```bash
cargo run -- config ghostfolio_url=http://localhost:3333
cargo run -- config access_token=YOUR_TOKEN
cargo run -- config anthropic_api_key=YOUR_KEY
cargo run -- config openrouter_api_key=YOUR_KEY
cargo run -- config openai_api_key=YOUR_KEY
cargo run -- config llm_provider=openrouter
cargo run -- config model=openai/gpt-4o-mini
cargo run -- config langchain_api_key=YOUR_KEY
cargo run -- config langchain_project=ghostfolio
```

### `evals`

Run suite:

```bash
cargo run -- evals run --suite quick
```

List suites:

```bash
cargo run -- evals run --list-suites
```

Case override:

```bash
cargo run -- evals run --suite quick --case acct-001,mkt-001
```

Model matrix:

```bash
cargo run -- evals run --suite quick --models openai/gpt-4o-mini,anthropic/claude-sonnet-4.6
```

Provider override:

```bash
cargo run -- evals run --suite quick --provider openrouter --model openai/gpt-4o-mini
```

Live API mode:

```bash
cargo run -- evals run --suite quick --live
```

Console mode (disable eval TUI):

```bash
cargo run -- evals run --suite quick --no-tui
```

Eval corpus definitions and suite files live under [`cli/evals/`](./evals/).

Report latest run (or explicit run ID):

```bash
cargo run -- evals report
cargo run -- evals report --run-id rust-run-20260227-123456
```

Inspect stored run artifacts:

```bash
cargo run -- evals get cli/evals/results/rust-run-20260227-123456
cargo run -- evals get cli/evals/results/rust-run-20260227-123456 --case act-001
cargo run -- evals get cli/evals/results/rust-run-20260227-123456/act-001.json
```

## Evals Output Modes

### Evals TUI (default)

When running in a terminal, evals open a dashboard with:

- Per-case rows (`PASS`/`FAIL`/`ERR`)
- Live tool trail per case
- Footer totals (completed, passed, failed, errors, elapsed)
- Detail modal per case (tool calls, tier failures, response/error, run ID)

Keys:

- `↑` / `↓`: move selection
- `Enter`: open case detail
- `q`: abort (while running) or exit (when done)
- Detail modal: `Esc` / `Enter` / `q` to close, `↑` / `↓` to scroll

### Console Output (`--no-tui`)

Plain text output includes:

- Per-case status line
- Tier failure details (`tier_a`, `tier_b`, `tier_c`) on failures
- Per-step trace with tool timings
- Run summary totals and per-model summary
- Cross-model diffs when running multiple models

## Result Artifacts

Eval runs write artifacts to:

- Per-case JSON: `cli/evals/results/<run-id>/<case-id>.json`
- SQLite summary DB: `cli/evals/results/results.db`

## Verification in CLI Agent

The agent now records structured verification on every response:

- `claim_to_tool_grounding`
- `tool_error_propagation`
- confidence score (`high|medium|low` + numeric score)

Secondary verifier (optional):

- Disabled by default
- Enable with env vars:
- `GF_VERIFY_PROVIDER` (`anthropic|openrouter|openai`)
- `GF_VERIFY_MODEL` (model ID)
- When enabled, it runs only for risky/low-confidence responses

## Config and Cache Paths

- Config file: `~/.config/ghostfolio-cli/config.toml`
- Legacy `config.json` is auto-migrated to TOML on load
- Provider model caches: `~/.config/ghostfolio-cli/providers/*.json`

API key pool env vars for eval parallelism:

- Comma-separated: `ANTHROPIC_API_KEYS`, `OPENROUTER_API_KEYS`, `OPENAI_API_KEYS`
- Numbered: `ANTHROPIC_API_KEY_1..20` (same pattern for OpenRouter/OpenAI)

## LangSmith / LangChain Tracing

Tracing is enabled when `LANGCHAIN_API_KEY` (or configured equivalent) is present.

Common env vars:

- `LANGCHAIN_API_KEY`
- `LANGCHAIN_PROJECT`
- `LANGCHAIN_ENDPOINT` (optional)

## Development and Tests

Run Rust tests:

```bash
cargo test
```

Compile tests only:

```bash
cargo test --no-run
```

`cargo test` vs `ghostfolio evals run`:

- `cargo test`: Rust unit/integration tests for implementation
- `ghostfolio evals run`: scenario-based behavioral grading of agent outputs
