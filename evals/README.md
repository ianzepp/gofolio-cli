# Ghostfolio Agent Eval Harness

An evaluation harness for testing the Ghostfolio AI agent across models, tools, and edge cases. The **Rust CLI runner** is the primary execution path. A legacy TypeScript runner remains available temporarily during migration.

Migration checklist and parity gates live in `MIGRATION_STATUS.md`.

## Architecture

```
evals/
  golden_sets/        Stage 1 — regression tests (happy-path, single-tool)
  scenarios/          Stage 2 — coverage mapping (multi-tool, adversarial, portfolio)
  rubrics/            Stage 4 — LLM-as-judge config (planned)
  suites.yaml         Cross-cutting suite definitions
  models.yaml         Model registry (38 models across 8+ providers)
  results/            NDJSON result files + results.db
  runner/             TypeScript runner implementation (legacy fallback)
    run.ts            CLI entry point, HTTP client, parallel orchestration
    graders.ts        Grading logic (Tiers A-C, Tier D planned)
    types.ts          TypeScript interfaces for all data structures
    db.ts             SQLite schema and insert functions
    seed-portfolio.ts Portfolio seeding helper
```

## Quick Start (Primary: Rust CLI)

```bash
# Run the default suite (comprehensive) using the Rust CLI runner
# (from gauntlet/)
npm run eval

# Or directly (from repo root)
cargo run --manifest-path cli/Cargo.toml -- test --suite comprehensive
```

### Common CLI Commands

```bash
# List suites from suites.yaml
cargo run --manifest-path cli/Cargo.toml -- test --list-suites

# Run a suite with fixture-backed mock tools (default mode)
cargo run --manifest-path cli/Cargo.toml -- test --suite quick --models openai/gpt-4o-mini,claude-sonnet-4-6 --parallel --max-parallel 4

# Run against live Ghostfolio API instead of fixtures
cargo run --manifest-path cli/Cargo.toml -- test --suite quick --live --model openai/gpt-4o-mini
```

Rust runner outputs:

- NDJSON: `evals/results/rust-run-*.jsonl`
- SQLite: `evals/results/results.db` (same `runs`/`results`/`steps` tables)

**Prerequisites**

- Rust toolchain
- Running Ghostfolio API server (default: `http://localhost:3333`)
- LLM keys configured for CLI (`ANTHROPIC_API_KEY`, `OPENROUTER_API_KEY`, and/or `OPENAI_API_KEY`)

## CLI Flags (Rust Runner)

| Flag             | Argument    | Default                | Description                                                  |
| ---------------- | ----------- | ---------------------- | ------------------------------------------------------------ |
| `--suite`        | name        | `quick`                | Test suite to run                                            |
| `--models`       | id1,id2     | configured model       | Comma-separated model IDs                                    |
| `--model`        | id          | configured model       | Single model ID (conflicts with `--models`)                  |
| `--provider`     | provider id | auto                   | Provider override (`anthropic`, `openrouter`, `openai`)      |
| `--case`         | id1,id2     | suite case selection   | Override suite with specific case IDs                        |
| `--parallel`     | —           | false                  | Run cases in parallel tasks                                  |
| `--max-parallel` | number      | CPU parallelism        | Max concurrent cases when `--parallel` is enabled            |
| `--list-suites`  | —           | —                      | Print available suites and exit                              |
| `--live`         | —           | false (fixture-backed) | Use live Ghostfolio API calls instead of local fixture mocks |

### Examples

```bash
# Quick smoke test on two models
cargo run --manifest-path cli/Cargo.toml -- test --suite quick --models openai/gpt-4o-mini,claude-sonnet-4-6

# Specific cases only
cargo run --manifest-path cli/Cargo.toml -- test --case acct-001,mkt-001,fx-001 --model openai/gpt-4o-mini

# Multi-model comparison with bounded parallelism
cargo run --manifest-path cli/Cargo.toml -- test --parallel --max-parallel 6 --models openai/gpt-4o-mini,claude-sonnet-4-6,google/gemini-2.5-flash
```

## Legacy TypeScript Runner (Fallback)

```bash
cd gauntlet
npm run eval:legacy
```

## Test Suites

Defined in `suites.yaml`:

| Suite           | Stage       | Cases | Fixture              | Description                                |
| --------------- | ----------- | ----- | -------------------- | ------------------------------------------ |
| `quick`         | golden_sets | 7     | `moderate-portfolio` | Fast smoke test — one case per tool        |
| `multi-tool`    | scenarios   | 4     | `moderate-portfolio` | Multi-tool chain tests                     |
| `adversarial`   | scenarios   | 2     | `moderate-portfolio` | Edge cases and prompt injection resistance |
| `regression`    | scenarios   | 6     | `moderate-portfolio` | Regression tests from known bugs           |
| `portfolio`     | scenarios   | 4     | `moderate-portfolio` | Portfolio analysis with seeded holdings    |
| `comprehensive` | all         | 25    | `moderate-portfolio` | Complete test coverage across all stages   |

## Test Case Format

Cases are YAML arrays in `golden_sets/*.yaml` and `scenarios/*.yaml`:

```yaml
- id: 'multi-003'
  description: 'Price + currency conversion in one request'
  query: 'How much is MSFT stock worth in GBP right now?'
  expected_tools: ['market_data', 'exchange_rate']
  must_contain: ['MSFT', 'GBP']
  must_not_contain: ['I cannot', 'unable to']
  expected_verified: true
  tags: ['multi-tool', 'regression']
```

| Field               | Type     | Description                                                 |
| ------------------- | -------- | ----------------------------------------------------------- |
| `id`                | string   | Unique case identifier (e.g. `fx-001`)                      |
| `description`       | string   | Human-readable description                                  |
| `query`             | string   | User message sent to the agent                              |
| `expected_tools`    | string[] | Tools that must be called (superset OK)                     |
| `must_contain`      | string[] | Strings that must appear in the response (case-insensitive) |
| `must_not_contain`  | string[] | Strings that must NOT appear (catches hallucinations)       |
| `expected_verified` | boolean  | Expected value of the agent's verification flag             |
| `tags`              | string[] | Categorization tags                                         |

### Case Organization

- **`golden_sets/`** — Stage 1 regression tests. Happy-path, single-tool cases that must all pass after every change.
  - `account.yaml`, `market.yaml`, `exchange.yaml`, `lookup.yaml`, `profile.yaml`, `benchmark.yaml`, `history.yaml`, `general.yaml`
- **`scenarios/`** — Stage 2 coverage mapping. Multi-tool, adversarial, and portfolio cases for release testing.
  - `multi_tool.yaml`, `adversarial.yaml`, `portfolio.yaml`

## Grading System

Every response is graded across three independent tiers. All three must pass for the case to pass.

### Tier A — Tool Selection

Checks that **all expected tools were called**. Extra tools beyond what's expected are allowed (superset OK). If no tools are expected and none are called, the tier passes.

### Tier B — Response Content

Checks that **all `must_contain` strings appear** in the agent's response text (case-insensitive substring search). Also checks that **no `must_not_contain` strings appear** — any match is a fail (catches hallucinations and refusal patterns).

### Tier C — Verification

Checks that the agent's `verified` flag matches `expected_verified`. The verification flag is a heuristic set by the agent service — data-oriented prompts that receive tool-backed answers are marked `verified=true`.

### Grade Output

```json
{
  "tierA": true,
  "tierB": true,
  "tierC": false,
  "pass": false,
  "details": {
    "tierA": "Exact tool match",
    "tierB": "All content assertions passed",
    "tierC": "Expected verified=true, got verified=false"
  }
}
```

## Execution Flow

1. **Parse CLI args** and load configs (`models.yaml`, `suites.yaml`, `golden_sets/*.yaml`, `scenarios/*.yaml`)
2. **Authenticate** — uses `EVAL_ACCESS_TOKEN` env var, or creates a throwaway eval user via the API
3. **Filter cases** by suite (or `--case` override)
4. **For each model**, for each case:
   - `POST /api/v1/agent/chat` with `{ message, model }`
   - Parse the NDJSON response stream (`step` events, then a `done` event)
   - Grade the response against expectations
   - Append result to NDJSON file and SQLite database
5. **Print per-model summary** (pass/fail/error counts, duration, tokens)
6. **Print comparison table** if multiple models were tested

### Parallel Mode

With `--parallel`, the runner spawns a child process per model. Each child runs the same script with internal flags (`--_auth-token`, `--_run-timestamp`) and writes its own NDJSON file. The parent collects results and prints a combined comparison table.

## Output

### NDJSON Files

Written to `results/run-{timestamp}-{model-label}.jsonl`. Each line is a JSON object:

- **Lines 1..N**: One `EvalResult` per test case with full grade, tool calls, steps, token usage, and timing
- **Final line**: `{ "_summary": { total, passed, failed, errors, durationMs, totalTokens, timestamp } }`

### SQLite Database

Stored at `results/results.db` (WAL mode). Three tables:

| Table     | Stores                                                              |
| --------- | ------------------------------------------------------------------- |
| `runs`    | Per-model run summaries (pass/fail counts, token totals, duration)  |
| `results` | Per-case results (tier grades, tools called, response text, errors) |
| `steps`   | Per-step execution details (tool calls, per-step token usage)       |

## Environment Variables

| Variable            | Default                 | Description                                     |
| ------------------- | ----------------------- | ----------------------------------------------- |
| `EVAL_BASE_URL`     | `http://localhost:3333` | API server URL                                  |
| `EVAL_ACCESS_TOKEN` | —                       | Pre-existing access token (skips user creation) |

## Models

38 models defined in `models.yaml`, identified by OpenRouter model IDs. Providers include Anthropic, Google, OpenAI, Meta, Mistral, Amazon, DeepSeek, x.ai, and others. Use `--list-models` to see all available labels.

## Adding a Test Case

1. Add a YAML entry to an existing file in `golden_sets/` (for single-tool regression tests) or `scenarios/` (for multi-tool/adversarial/portfolio tests)
2. Include all required fields: `id`, `description`, `query`, `expected_tools`, `must_contain`, `must_not_contain`, `expected_verified`, `tags`
3. Add the case ID to the appropriate suite(s) in `suites.yaml` (or rely on `comprehensive` which runs all cases)

## Adding a Model

Add an entry to `models.yaml`:

```yaml
- id: provider/model-name
  label: short-label
```

The `id` must be a valid OpenRouter model ID. The `label` is what you pass to `--models`.
