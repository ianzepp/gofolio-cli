# Ghostfolio CLI (Rust)

`cli/` is the Rust application for Ghostfolio's terminal UI and agent runtime.

## What It Includes

- Interactive terminal UI (`ghostfolio` / `ghostfolio chat`)
- Agent orchestration + tool calls
- Config management (`ghostfolio config`)
- Eval runner (`ghostfolio evals`)
- Unit tests and coverage for Rust code

## Prerequisites

- Rust toolchain (`cargo`, `rustc`)
- Ghostfolio API reachable (default `http://localhost:3333`)
- API token and model provider keys configured

## Quick Start

```bash
cd cli
cargo run
```

The default command opens chat mode.

## CLI Commands

### `chat` (default)

Start the interactive UI:

```bash
cargo run -- chat
# or simply
cargo run
```

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
cargo run -- config model=openai/gpt-4o-mini
cargo run -- config llm_provider=openrouter
```

### `evals`

Run eval suites against the in-process CLI agent:

```bash
cargo run -- evals --suite quick
```

List suites:

```bash
cargo run -- evals --list-suites
```

Multi-model run:

```bash
cargo run -- evals --suite quick --models openai/gpt-4o-mini,claude-sonnet-4-6 --parallel --max-parallel 4
```

Live API mode (instead of fixtures):

```bash
cargo run -- evals --suite quick --live --model openai/gpt-4o-mini
```

Eval corpus details: [evals/README.md](./evals/README.md)

## Config and Provider Caches

Config file path:

- `~/.config/ghostfolio-cli/config.json`

Provider model caches:

- `~/.config/ghostfolio-cli/providers/*.json`

These cached provider lists are used for model selection workflows.

## Rust Testing

### Unit tests

All unit tests live in `*_test.rs` files.

```bash
cd cli
cargo test
```

Run a subset:

```bash
cargo test tools::calculator
```

### Coverage

```bash
cargo llvm-cov --summary-only
```

## Distinction: `cargo test` vs `ghostfolio evals`

- `cargo test`: Rust unit tests for implementation code
- `ghostfolio evals`: scenario/golden-set behavioral evaluation harness
