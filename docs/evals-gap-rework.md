# Evals Gap Analysis & Rework Plan

## Implementation Status

| Phase | Priority | Work Item                                   | Status      | Commit    |
| ----- | -------- | ------------------------------------------- | ----------- | --------- |
| 1     | **P0**   | Rename fields (`prompt`→`query`, etc.)      | **Done**    | e35fd18df |
| 1     | **P0**   | Add `must_not_contain` + grader check       | **Done**    | e35fd18df |
| 1     | **P0**   | Restructure directories                     | **Done**    | e35fd18df |
| 1     | **P0**   | Update `suites.yaml` with `stage` field     | **Done**    | e35fd18df |
| 1     | **P1**   | Add `category` + `difficulty` to all cases  | **Done**    | 03256a250 |
| 1     | **P1**   | Coverage matrix in summary output           | **Done**    | 03256a250 |
| 1     | **P1**   | Create `rubrics/rubrics.yaml` scaffold      | **Done**    | 03256a250 |
| 1     | **P2**   | Implement Tier D rubric scorer              | Not started | —         |
| 1     | **P2**   | Cost estimation + p50/p95 latency           | **Done**    | —         |
| 1     | **P3**   | `--replay` flag                             | Not started | —         |
| 2     | —        | CLI multi-provider support                  | **Done**    | bfc46e72b |
| 3     | —        | Rust eval runner with mock layer            | **Done**    | 0f7c365df |
| 3     | —        | Shared agent-loop reuse in eval runner      | **Done**    | ea21d6f15 |
| 3     | —        | Parallel case execution (`--parallel`)      | **Done**    | 86610a813 |
| 3     | —        | Model matrix + bounded parallelism          | **Done**    | cc52683e0 |
| 3     | —        | LangSmith tracing in eval runner            | **Done**    | e75c8e6b4 |
| 3     | —        | CLI-first entrypoint (`npm run eval`)       | **Done**    | cf57648d3 |
| 3     | —        | Eval definitions vendored into `cli/evals/` | **Done**    | e424f3443 |
| 3     | —        | Legacy TS runner retained as fallback       | **Done**    | cf57648d3 |

### Deviations from Original Plan

1. **`regression.yaml` not created** — The plan proposed a `scenarios/regression.yaml` file. Instead, regression cases remain in their domain files (`exchange.yaml`, `general.yaml`, etc.) and are selected by the `regression` suite in `suites.yaml`. This avoids duplicating cases across files.

2. **`suites.yaml` `stage` field done in P0, not P3** — The plan listed this as P3, but it was a natural part of the P0 directory restructure since the suites needed updating anyway.

3. **`adversarial` suite keeps `gen-001`** — The plan showed only `[adv-002]` in the adversarial suite. The implementation preserves the original `[gen-001, adv-002]` from the pre-rework suites.yaml since gen-001 (no-tools question) is a valid adversarial/edge case.

4. **`category` and `difficulty` carried to `EvalResult`** — The plan didn't specify this, but the fields are now on `EvalResult` too (not just `EvalCase`) so the coverage matrix can use them from results.

5. **Case count is 25, not 22** — The original doc stated 22 test cases. The actual count was already 25 before the rework (11 files, 25 cases). Updated throughout.

---

## Context

This document compares our existing eval harness (`gauntlet/evals/`) against the reference framework published by the Gauntlet program (`prod-evals-cookbook/`). The cookbook defines a 5-stage evaluation pipeline that class graders use as their baseline lens. We are not required to follow it exactly, but aligning our structure and vocabulary reduces friction during grading and surfaces genuine gaps in our evaluation depth.

A secondary goal: the Rust CLI (`cli/`) will eventually be extracted into a standalone repo, and the evals directory should travel with it. The restructured layout must be self-contained and transport-agnostic — YAML definitions are the contract, the runner is an implementation detail.

### Reference Sources

- **Our evals**: `ghostfolio/cli/evals/` — TypeScript runner, 25 test cases, 3-tier grading, 38 models, NDJSON + SQLite output
- **Cookbook**: `prod-evals-cookbook/` — Python, 5-stage pipeline (golden sets → labeled scenarios → replay harnesses → rubrics → experiments)

---

## 1. What We Already Cover Well

> **Status: Baseline established. Sections 2-7 track gap closures.**

| Cookbook Concept                        | Our Implementation                                              | Assessment             |
| --------------------------------------- | --------------------------------------------------------------- | ---------------------- |
| Stage 1: Golden sets (regression tests) | `golden_sets/*.yaml` with `expected_tools`, `must_contain`      | Strong match           |
| Tool selection checks                   | Tier A grading (superset-OK)                                    | Strong match           |
| Content validation                      | Tier B (`must_contain` + `must_not_contain`)                    | Strong match           |
| Stage 5: Multi-model experiments        | `models.yaml` (38 models), `--parallel` mode, comparison tables | We exceed the cookbook |
| Test data seeding                       | `seed-portfolio.ts` with lognormal price simulation             | Strong match           |
| Dual persistence                        | NDJSON (streaming) + SQLite (queryable)                         | Exceeds cookbook       |

### What We Do Better

1. **Multi-model at scale** — 38 models across 8 providers; cookbook only tests OpenAI variants
2. **Parallel execution** — child-process-per-model with coordinated timestamps
3. **Verification tier** — Tier C (agent self-reports confidence) is unique to us
4. **Realistic portfolio seeding** — geometric Brownian motion price simulation
5. **Dual persistence** — NDJSON + SQLite vs. cookbook's simpler output

---

## 2. Gaps

### Gap 1: No Negative Assertions (`must_not_contain`) — CLOSED

> **Status: Done** (P0, commit e35fd18df)

**Cookbook**: Cases include `must_not_contain` to catch hallucination markers (e.g., `"I don't know"`, fabricated data).

**Implementation**: Added `must_not_contain: string[]` field to `EvalCase`. Tier B grader checks both `must_contain` (all must be present) and `must_not_contain` (any match is a fail). All 25 cases initialized with `must_not_contain: []` — values to be populated as hallucination patterns are identified.

### Gap 2: No Category/Difficulty Taxonomy or Coverage Matrix — CLOSED

> **Status: Done** (P1, commit 03256a250)

**Cookbook**: Cases are organized along three axes — tool type (`vector_only`, `sql_only`), complexity (`single_tool`, `multi_tool`), and difficulty (`straightforward`, `ambiguous`, `edge_case`). Output includes a coverage heatmap.

**Implementation**: Added `category` (11 values) and `difficulty` (3 values: `straightforward`, `ambiguous`, `edge_case`) fields to all cases. Coverage matrix prints after each model run showing pass rates by category × difficulty with row/column totals.

### Gap 3: No LLM-as-Judge / Rubric Scoring — PARTIALLY CLOSED

> **Status: Scaffold done** (P1, commit 03256a250). **Tier D scorer not yet implemented** (P2).

**Cookbook (Stages 3 & 4)**: Uses LLM-as-judge to score responses across weighted dimensions:

- **Relevance** (weight 0.25) — Does it address the question?
- **Accuracy** (weight 0.35) — Factually correct?
- **Completeness** (weight 0.25) — All aspects covered?
- **Clarity** (weight 0.15) — Well-organized?

Category-specific weight overrides (e.g., accuracy weighted 0.45 for policy questions). Quality thresholds from "Critical" (<1.5) to "Excellent" (4.5-5.0).

**Implementation so far**: Created `rubrics/rubrics.yaml` with all 4 dimensions, weights, 0-5 scales, and quality thresholds. Tier D scorer (calling an LLM to grade responses) is planned for P2.

### Gap 4: No Replay / Session Recording — OPEN

> **Status: Not started** (P3)

**Cookbook (Stage 3)**: Records full sessions as JSON fixtures (query → tool calls → sources → response). Replays them without LLM calls for deterministic, cost-free re-evaluation.

**Us**: Every eval run hits the live agent. No replay capability.

**Risk**: Low-medium. Nice-to-have for reproducibility, not a must-have.

**Fix**: We already persist full traces in NDJSON + SQLite. A `--replay` flag that loads prior results and re-grades them would close this gap without a separate directory. Lower priority.

### Gap 5: No Source Citation Checks — OPEN (Low Priority)

> **Status: Not started.** May not apply to our domain.

**Cookbook**: Cases include `expected_sources` — which documents the agent should have cited.

**Us**: Not checked. May not apply if our agent doesn't surface source metadata.

**Risk**: Low (domain-dependent).

**Fix**: Check if agent responses include source information. If so, add to grading.

### Gap 6: No Cost/Latency Percentiles in Summary — CLOSED

> **Status: Done** (P2)

**Cookbook**: Experiment reports include latency (p50, p95) and estimated cost per variant.

**Implementation**: Added `cost_prompt` and `cost_completion` fields to `models.yaml` (sourced from OpenRouter pricing in `models-upstream.json`). Comparison table now includes p50/p95 latency (in seconds) and estimated cost (USD) columns. Per-model summary line also shows estimated cost. `ModelRunSummary` and `EvalSummary` types extended with `p50LatencyMs`, `p95LatencyMs`, and `estimatedCostUsd` fields.

---

## 3. Naming Renames — DONE

> **Status: All renames implemented** (P0, commit e35fd18df)

| Field               | Before                 | After               | Status    |
| ------------------- | ---------------------- | ------------------- | --------- |
| Question            | `prompt`               | `query`             | **Done**  |
| Content assertions  | `expected_in_response` | `must_contain`      | **Done**  |
| Negative assertions | _(missing)_            | `must_not_contain`  | **Done**  |
| Difficulty          | _(missing)_            | `difficulty`        | **Done**  |
| Category            | _(derived from tags)_  | `category`          | **Done**  |
| Tool expectations   | `expected_tools`       | `expected_tools`    | No change |
| Verification        | `expected_verified`    | `expected_verified` | No change |
| Tags                | `tags`                 | `tags`              | No change |
| Case description    | `description`          | `description`       | No change |

### Case IDs — No Change

Our domain-based prefixes (`mkt-001`, `fx-002`, `port-003`) are more descriptive than the cookbook's stage-based prefixes (`gs-001`, `sc-v-001`). A grader can tell what `mkt-001` tests at a glance. Keep as-is.

### Implemented Case Format

```yaml
- id: 'mkt-001'
  description: 'Stock price query'
  query: 'What is the current price of AAPL?'
  category: 'market_data'
  difficulty: 'straightforward'
  expected_tools: ['market_data']
  must_contain: ['AAPL']
  must_not_contain: []
  expected_verified: true
  tags: ['happy-path', 'market']
```

---

## 4. Directory Restructure — DONE

> **Status: Implemented** (P0, commit e35fd18df)

### Previous Layout (flat, function-oriented)

```
evals/
├── cases/                          # All test cases (by domain)
│   ├── market.yaml
│   ├── exchange.yaml
│   └── ... (11 files)
├── results/                        # All output (flat)
│   ├── results.db
│   └── run-*.jsonl
├── types.ts
├── graders.ts
├── run.ts
├── db.ts
├── seed-portfolio.ts
├── suites.yaml
├── models.yaml
└── README.md
```

### Current Layout (stage-aligned, runner-isolated)

```
evals/
├── README.md
│
├── golden_sets/                    # Stage 1: Regression tests
│   ├── account.yaml                #   Happy-path, single-tool cases
│   ├── market.yaml                 #   One file per domain
│   ├── exchange.yaml               #   (includes multi-tool fx-003/004)
│   ├── lookup.yaml
│   ├── profile.yaml
│   ├── benchmark.yaml
│   ├── history.yaml
│   └── general.yaml
│
├── scenarios/                      # Stage 2: Coverage mapping
│   ├── multi_tool.yaml             #   Multi-tool chaining
│   ├── adversarial.yaml            #   Prompt injection resistance
│   └── portfolio.yaml              #   Seeded portfolio queries
│
├── rubrics/                        # Stage 4: LLM-as-judge config
│   └── rubrics.yaml                #   Scoring dimensions + weights
│
├── suites.yaml                     # Cross-cutting: suite → case mapping
├── models.yaml                     # Cross-cutting: model registry
│
├── results/                        # Stage 5: Experiment output
│   ├── results.db
│   └── run-*.jsonl
│
├── runner/                         # Runner implementation (swappable)
│   ├── run.ts                      #   CLI entry point & orchestrator
│   ├── graders.ts                  #   Grading logic (Tiers A-C)
│   ├── db.ts                       #   SQLite persistence
│   ├── seed-portfolio.ts           #   Portfolio setup
│   └── types.ts                    #   Data model interfaces
│
└── models-upstream.json            # Reference data
```

**Deviation from plan**: The proposed `scenarios/regression.yaml` was not created. Regression cases stay in their domain files (e.g., `fx-003` in `golden_sets/exchange.yaml`, `gen-002` in `golden_sets/general.yaml`) and are selected via the `regression` suite in `suites.yaml`. This avoids duplicating cases.

### Design Rationale

**Why `golden_sets/` vs `scenarios/` split?**

Golden sets contain happy-path, single-tool cases — the tests you run after every change to catch regressions. All must pass. Scenarios contain multi-tool, adversarial, portfolio, and regression cases — the tests you run on releases to map coverage. Some failures are acceptable.

This maps directly to the cookbook's Stage 1 (must-pass regression) vs Stage 2 (coverage mapping with thresholds).

**Why `runner/` subdirectory?**

Isolates the TypeScript-specific runner code. The YAML files in `golden_sets/`, `scenarios/`, `rubrics/`, `suites.yaml`, and `models.yaml` are the contract — they're language-agnostic. When the Rust CLI uses `cargo run -- evals`, it reads the same YAML but implements its own runner logic. Two runners, one test suite.

**Why no `stage_3_replay_harnesses/`?**

We already persist full execution traces (tool calls, steps, tokens, responses) in NDJSON and SQLite. Replay is a runner feature (`--replay` flag), not a directory of fixtures. Lower priority, and doesn't need structural changes.

**Why `suites.yaml` and `models.yaml` at root?**

They reference files across `golden_sets/` and `scenarios/`, so they sit at the evals root as cross-cutting config rather than belonging to a single stage.

---

## 5. Suite Definitions — DONE

> **Status: Implemented** (P0, commit e35fd18df)

Current `suites.yaml`:

```yaml
# Stage 1 suites — run on every change, all must pass
quick:
  description: 'Fast smoke test — one case per tool category'
  stage: golden_sets
  cases: [acct-001, mkt-001, fx-001, sym-001, prof-001, bench-001, hist-001]

# Stage 2 suites — run on releases, coverage mapping
multi-tool:
  description: 'Multi-tool chain tests'
  stage: scenarios
  cases: [multi-001, multi-002, multi-003, multi-004]

adversarial:
  description: 'Edge cases and prompt injection resistance'
  stage: scenarios
  cases: [gen-001, adv-002]

regression:
  description: 'Regression tests from known bugs'
  stage: scenarios
  cases: [gen-002, gen-003, fx-003, fx-004, multi-003, multi-004]

portfolio:
  description: 'Portfolio analysis with seeded holdings'
  stage: scenarios
  cases: [port-001, port-002, port-003, port-004]
  setup:
    portfolio:
      risk: moderate
      size: 500000

# Full suite — all stages
comprehensive:
  description: 'Complete test coverage across all stages'
  stage: all
  cases: all
  setup:
    portfolio:
      risk: moderate
      size: 500000
```

**Deviation from plan**: The adversarial suite includes `gen-001` (preserved from the original suites.yaml) in addition to `adv-002`. The plan had only `adv-002`.

---

## 6. Rubric Scaffold — DONE

> **Status: File created** (P1, commit 03256a250). Tier D scorer that reads this file is P2.

`rubrics/rubrics.yaml` — implemented exactly as planned:

```yaml
version: '1.0'

dimensions:
  relevance:
    weight: 0.25
    description: 'Does the response address the question?'
    scale:
      5: 'Directly and completely addresses the question'
      3: 'Main point covered but includes unnecessary information'
      1: 'Barely related to the question'
      0: 'Off-topic or refuses to answer'

  accuracy:
    weight: 0.35
    description: 'Are the facts correct and verifiable from tool results?'
    scale:
      5: 'All facts correct, consistent with tool-returned data'
      3: 'Mostly correct, minor imprecisions'
      1: 'Significant factual errors'
      0: 'Completely wrong or fabricated data'

  completeness:
    weight: 0.25
    description: 'Does the response fully answer all parts of the question?'
    scale:
      5: 'Comprehensive — all aspects covered'
      3: 'Key points covered, some details missing'
      1: 'Minimal answer with major gaps'
      0: 'No substantive information provided'

  clarity:
    weight: 0.15
    description: 'Is the response well-organized and appropriately concise?'
    scale:
      5: 'Crystal clear, well-structured, appropriate length'
      3: 'Understandable but could be better organized'
      1: 'Difficult to follow'
      0: 'Incomprehensible'

thresholds:
  excellent: 4.5
  good: 3.5
  acceptable: 2.5
  poor: 1.5
```

---

## 7. Grading Changes — PARTIALLY DONE

> **Status: Tiers A-C done** (P0). **Tier D not yet implemented** (P2).

### Previous: Tiers A, B, C (deterministic)

| Tier | Checks            | Mechanism                                |
| ---- | ----------------- | ---------------------------------------- |
| A    | Tool selection    | Set comparison (superset OK)             |
| B    | Response content  | Substring match (`expected_in_response`) |
| C    | Verification flag | Boolean equality                         |

### Current: Tiers A, B, C (deterministic, enhanced)

| Tier | Checks            | Mechanism                                                                 | Status   |
| ---- | ----------------- | ------------------------------------------------------------------------- | -------- |
| A    | Tool selection    | Set comparison (superset OK) — unchanged                                  | **Done** |
| B    | Response content  | Substring match for `must_contain` + absence check for `must_not_contain` | **Done** |
| C    | Verification flag | Boolean equality — unchanged                                              | **Done** |
| D    | Quality rubric    | LLM-as-judge scoring across 4 dimensions                                  | P2       |

**Tier D behavior (planned for P2):**

- Calls a fast/cheap LLM (e.g., Haiku) with the query, response, and rubric prompt
- Returns per-dimension scores (0-5) and weighted overall score
- Does not affect pass/fail (Tiers A-C determine pass/fail)
- Reported separately as a quality metric for trending and comparison
- Can be skipped with `--no-rubrics` flag for fast runs

---

## 8. Future: Rust CLI Integration

> **Status: Done** (Phase 3 complete — Rust eval runner is feature-complete and primary)

When `cli/` becomes a standalone repo, the evals directory travels with it:

```
ghostfolio-cli/
├── src/                        # Rust source
├── Cargo.toml
└── evals/                      # Same structure as above
    ├── golden_sets/
    ├── scenarios/
    ├── rubrics/
    ├── suites.yaml
    ├── models.yaml
    ├── results/
    └── runner/                 # Rust runner replaces TS runner
```

The Rust CLI `test` subcommand is now the primary eval runner:

```bash
# Run golden sets (Stage 1)
cargo run -- evals --suite quick --model sonnet-4.6

# Run all scenarios (Stage 2)
cargo run -- evals --suite comprehensive --parallel

# Specific cases
cargo run -- evals --case mkt-001,fx-003

# Multi-model matrix comparison
cargo run -- evals --models anthropic/claude-sonnet-4.6,openai/gpt-4o --parallel

# With LangSmith tracing (auto-detected from LANGCHAIN_API_KEY)
LANGCHAIN_API_KEY=... cargo run -- evals --suite quick
```

The runner:

1. Reads `evals/suites.yaml` → resolves case files from `golden_sets/` and `scenarios/`
2. Runs each case through the CLI's embedded agent (no HTTP — direct Rust call)
3. Uses `ToolDispatcher` enum (Live or Mock) for deterministic fixture-backed or live API testing
4. Grades using Tiers A-C (reimplemented in Rust, logic identical to TS)
5. Writes results to `evals/results/` (NDJSON + SQLite, identical schema)
6. Optionally traces to LangSmith when configured

**The YAML files are the shared contract. The runner is an implementation detail.**

Eval definitions are now vendored in `cli/evals/` (primary) with single-source `cli/evals/`.

---

## 9. CLI Multi-Provider Support (Prerequisite for Rust Eval Runner)

> **Status: Done** (Phase 2, completed on February 26, 2026; key commits: bfc46e72b, 4c1441696, ba80e315c, 04d8ac178)

### Current State

The Rust CLI now supports multiple providers and adapters:

- `cli/src/agent/client/mod.rs` includes provider-aware enum dispatch via `LlmClient`
- Supported providers: `Anthropic`, `OpenRouter`, `OpenAI`
- Supported adapters: `anthropic_messages`, `openai_chat_completions`, `openai_messages`
- Config resolves `ANTHROPIC_API_KEY`, `OPENROUTER_API_KEY`, and `OPENAI_API_KEY` (env or config file)
- Provider preference is configurable (`GHOSTFOLIO_LLM_PROVIDER` / `llm_provider`)
- Per-provider model lists are cached under `~/.config/ghostfolio-cli/providers/*.json` with API fallback
- UI now shows provider key status and grouped model picker by provider with live filtering

### Problem

Phase 2 removed the prior blocker (Anthropic-only CLI). The remaining blocker for Rust-native evals is still Phase 3: the Rust eval runner + mock tool layer.

### Implemented: Provider Selection from Environment/Config

The CLI config now builds a provider list from available credentials and chooses a preferred provider when configured:

| Env Variable         | Provider   | API Base URL                                    | API Pattern            |
| -------------------- | ---------- | ----------------------------------------------- | ---------------------- |
| `ANTHROPIC_API_KEY`  | Anthropic  | `https://api.anthropic.com/v1/messages`         | Anthropic Messages API |
| `OPENROUTER_API_KEY` | OpenRouter | `https://openrouter.ai/api/v1/chat/completions` | OpenAI-compatible      |
| `OPENAI_API_KEY`     | OpenAI     | `https://api.openai.com/v1/chat/completions`    | OpenAI-compatible      |
| `VERCEL_API_KEY`     | Vercel AI  | _(planned, not implemented)_                    | OpenAI-compatible      |
| `XAI_API_KEY`        | xAI (Grok) | _(planned, not implemented)_                    | OpenAI-compatible      |

**Key implementation detail**: Anthropic uses a native client; OpenRouter/OpenAI use a shared OpenAI-compatible client.

1. **Anthropic client** (`cli/src/agent/client/anthropic.rs`) — Anthropic-native format
2. **OpenAI-compatible client** (`cli/src/agent/client/openai.rs`) — currently used by OpenRouter and OpenAI

### Architecture

Implemented architecture in `cli/`:

- `LlmClient` enum dispatches requests to Anthropic or OpenAI-compatible clients
- `ProviderConfig` carries `{ provider, adapter, api_key }` and is built from env/config keys
- `Config::configured_llm_providers()` resolves available providers
- `Config::preferred_llm_provider()` selects active provider (explicit preference first, else first configured)
- `app.rs` holds one client per configured provider and routes chat calls through the active provider/model

### Translation Layer

The main work is mapping between Anthropic's content block format (which the CLI uses internally) and OpenAI's message format:

| Concept               | Anthropic Format                                    | OpenAI Format                                                               |
| --------------------- | --------------------------------------------------- | --------------------------------------------------------------------------- |
| System prompt         | Top-level `system` field                            | `{ role: "system", content: "..." }`                                        |
| Tool definition       | `{ name, description, input_schema }`               | `{ type: "function", function: { name, description, parameters } }`         |
| Tool call (response)  | `ContentBlock::ToolUse { id, name, input }`         | `{ tool_calls: [{ id, type: "function", function: { name, arguments } }] }` |
| Tool result (request) | `ContentBlock::ToolResult { tool_use_id, content }` | `{ role: "tool", tool_call_id, content }`                                   |
| Stop reason           | `"end_turn"`, `"tool_use"`                          | `"stop"`, `"tool_calls"`                                                    |
| Token usage           | `usage.input_tokens`, `usage.output_tokens`         | `usage.prompt_tokens`, `usage.completion_tokens`                            |

The OpenAI-compatible client translates outbound requests from Anthropic format → OpenAI format, and inbound responses from OpenAI format → Anthropic's `ChatResponse` type. The rest of the CLI (agent loop, tool dispatch, UI) sees no difference.

### Scope & Priority

This CLI enhancement is now complete and unblocks the eventual migration of evals from TS to Rust:

1. **Done**: Evals restructure (this document, sections 1-8)
2. **Done**: CLI multi-provider support (this section)
3. **Next**: Rust eval runner with mock HTTP layer (section 10)

### Implementation Effort

| Work Item                                       | Effort | Notes                                                      |
| ----------------------------------------------- | ------ | ---------------------------------------------------------- |
| Add provider-aware client dispatch              | Low    | Done via `LlmClient` enum                                  |
| Add `OpenAIClient` with format translation      | Medium | Done                                                       |
| Add provider selection from env/config keys     | Low    | Done (`ANTHROPIC` / `OPENROUTER` / `OPENAI`)               |
| Wire provider selection into `app.rs`           | Low    | Done                                                       |
| Add provider status + grouped model picker UX   | Low    | Done (key format checks, grouped selector, provider cache) |
| Add `--provider` CLI flag for explicit override | Low    | Deferred                                                   |

---

## 10. Mock HTTP Layer & Hallucination Testing

> **Status: Done** (Phase 3, completed February 26, 2026)

### The Architectural Shift

The current TS eval runner round-trips through the **live Ghostfolio server**:

```
TS Runner → POST /api/v1/agent/chat → Ghostfolio Backend → LLM → Tool Calls → Ghostfolio API → Response
                                       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
                                       All of this requires a running Ghostfolio server with
                                       real data, real accounts, real market connections
```

The CLI-based eval runner changes this fundamentally. The agent loop lives **inside the CLI binary**. Tool dispatch calls `GhostfolioClient` which is just an HTTP client calling Ghostfolio REST endpoints. The seam is clean:

```
CLI Eval Runner → Agent Loop (in-process) → LLM API → Tool Calls → GhostfolioClient → HTTP → ???
                  ^^^^^^^^^^^^^^^^^^^^^^^^                          ^^^^^^^^^^^^^^^^^
                  Controlled by us                                  This is the mock point
```

By intercepting at the `GhostfolioClient` / HTTP layer, we can replace the live Ghostfolio server with **mock HTTP responses** containing known, deterministic data. The LLM still runs for real (that's what we're testing), but the data it receives from tools is fully controlled.

### What This Enables

#### 1. No Server Required

Tests run with only an LLM API key. No Docker, no Ghostfolio instance, no database, no market data connections. This makes evals:

- Runnable in CI without infrastructure
- Runnable on any developer machine
- Independent of Ghostfolio release cycles

#### 2. Deterministic Tool Data

Every tool call returns the same data every run. No flaky tests from market data changes or account state drift.

#### 3. Hallucination Detection (Accuracy Verification)

This is the big unlock. Because we control the source data, we can verify the LLM's response against **ground truth**. If the fixture says the portfolio is worth `$127,432.15` and the LLM says `$127,000`, that's a grading fail. If the LLM mentions a holding that doesn't exist in the fixture, that's a hallucination.

**This was impossible with the live server** because we couldn't predict what data the tools would return at test time, so we couldn't write precise content assertions.

#### 4. Edge Case Testing Without Real Data

We can create fixture sets for scenarios that are hard to set up in a live system:

- Empty portfolios (no holdings)
- Extremely large portfolios (1000+ holdings)
- Negative returns, zero-value positions
- Currency mismatches
- API errors (tool returns error — does the LLM handle it gracefully?)
- Missing fields (partial data — does the LLM hallucinate the gaps?)

#### 5. Rubric Scoring with Ground Truth

The LLM-as-judge rubric scorer (Tier D) becomes much more powerful when it has access to the fixture data. The judge can verify not just "does it sound right" but "does it match the data."

### Design: Golden Fixture Sets (Not Per-Case Mock Data)

Rather than embedding mock data in every test case (which would be verbose and hard to maintain), the mock layer loads **golden fixture sets** — complete snapshots of what the Ghostfolio API returns for every endpoint. Each fixture set represents a coherent "world state" that all test cases in a suite run against.

#### Fixture Set Structure

A fixture set is a directory containing one JSON file per API endpoint/tool:

```
evals/
├── fixtures/
│   ├── moderate-portfolio/             # A complete world state
│   │   ├── get_portfolio_summary.json  # GET /api/v1/portfolio/details
│   │   ├── get_holdings.json           # GET /api/v1/portfolio/holdings
│   │   ├── get_performance.json        # GET /api/v1/portfolio/performance
│   │   ├── list_accounts.json          # GET /api/v1/account
│   │   ├── list_activities.json        # GET /api/v1/order
│   │   ├── get_benchmarks.json         # GET /api/v1/benchmarks
│   │   ├── search_assets.json          # Keyed by query parameter
│   │   ├── get_asset_profile.json      # Keyed by symbol
│   │   ├── get_market_data.json        # GET /api/v1/admin/market-data
│   │   └── _manifest.yaml             # Metadata about this fixture set
│   │
│   ├── empty-portfolio/                # Edge case: no holdings
│   │   ├── get_portfolio_summary.json
│   │   ├── get_holdings.json           # Returns empty array
│   │   └── ...
│   │
│   ├── large-portfolio/                # Stress test: 100+ holdings
│   │   └── ...
│   │
│   └── error-states/                   # API error responses
│       ├── get_portfolio_summary.json  # Returns 500 error
│       └── ...
```

#### Fixture Manifest

Each fixture set includes a `_manifest.yaml` describing the world state, so test case authors know what data is available:

```yaml
name: moderate-portfolio
description: >
  $500k moderate-risk portfolio with 8 holdings across US stocks,
  international ETFs, bonds, and crypto. 12 months of activity history.
  Two accounts: main brokerage + crypto wallet.

accounts:
  - id: 'acct-main'
    name: 'Main Brokerage'
    balance: 425000.00
    currency: 'USD'
  - id: 'acct-crypto'
    name: 'Crypto Wallet'
    balance: 75000.00
    currency: 'USD'

holdings_summary:
  count: 8
  symbols: ['AAPL', 'MSFT', 'GOOGL', 'VEA', 'BND', 'AGG', 'BTCUSD', 'ETHUSD']
  total_value: 500432.15

activities:
  count: 24
  date_range: '2025-03-01 to 2026-02-15'
  types: ['BUY', 'SELL', 'DIVIDEND']

key_facts:
  # These are the ground-truth values test cases should assert against
  - 'Total portfolio value is $500,432.15'
  - 'AAPL: 150 shares at $198.45 = $29,767.50'
  - 'MSFT: 80 shares at $425.12 = $34,009.60'
  - 'BTCUSD: 0.75 BTC at $63,250.00 = $47,437.50'
  - 'YTD return is +8.3%'
  - 'Total dividends received: $1,247.33'
```

#### Parameterized Tool Responses

Some tools accept parameters (e.g., `search_assets` takes a query, `get_holding_detail` takes a symbol). The fixture JSON can be keyed by parameter:

```json
// fixtures/moderate-portfolio/get_holding_detail.json
{
  "AAPL": {
    "symbol": "AAPL",
    "name": "Apple Inc.",
    "quantity": 150,
    "marketPrice": 198.45,
    "averageCost": 172.3,
    "value": 29767.5,
    "performance": { "percent": 15.17, "value": 3922.5 }
  },
  "MSFT": {
    "symbol": "MSFT",
    "name": "Microsoft Corporation",
    "quantity": 80,
    "marketPrice": 425.12,
    "averageCost": 388.5,
    "value": 34009.6,
    "performance": { "percent": 9.42, "value": 2929.6 }
  }
}
```

The mock dispatcher looks up the appropriate key based on the tool's input parameters:

```rust
pub async fn dispatch_mock(
    fixture: &FixtureSet,
    tool_name: &str,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let data = fixture.get(tool_name)
        .ok_or_else(|| ApiError::Request(format!("no fixture for tool: {tool_name}")))?;

    // If fixture is a keyed object and input has a lookup key, resolve it
    if let Some(key) = extract_lookup_key(tool_name, input) {
        data.get(&key)
            .cloned()
            .ok_or_else(|| ApiError::Request(format!("no fixture data for {tool_name}[{key}]")))
    } else {
        Ok(data.clone())
    }
}
```

#### Suite → Fixture Binding

Suites reference which fixture set to use. All cases in the suite run against the same world state:

```yaml
# suites.yaml (future, when fixtures are implemented)
quick:
  description: 'Fast smoke test — one case per tool category'
  stage: golden_sets
  fixture: moderate-portfolio # All cases see this data
  cases: [acct-001, mkt-001, fx-001, sym-001, prof-001, bench-001, hist-001]

edge-empty:
  description: 'Edge case — empty portfolio'
  stage: scenarios
  fixture: empty-portfolio
  cases: [edge-empty-001, edge-empty-002]

edge-errors:
  description: 'Edge case — API errors'
  stage: scenarios
  fixture: error-states
  cases: [err-001, err-002]

comprehensive:
  description: 'Complete test coverage'
  stage: all
  fixture: moderate-portfolio
  cases: all
  setup:
    portfolio:
      risk: moderate
      size: 500000
```

#### Test Cases Become Simpler

With fixture sets loaded at the suite level, test cases don't carry any mock data. They just assert against known values from the fixture:

```yaml
- id: 'port-001'
  description: 'Portfolio total value'
  query: 'What is my total portfolio value?'
  category: 'portfolio'
  difficulty: 'straightforward'
  expected_tools: ['get_portfolio_summary']
  must_contain: ['500,432.15']
  must_not_contain: ['500,000', 'approximately']
  expected_verified: true
  tags: ['happy-path', 'portfolio']

- id: 'port-002'
  description: 'Individual holding detail'
  query: 'How is my Apple stock doing?'
  category: 'portfolio'
  difficulty: 'straightforward'
  expected_tools: ['get_holding_detail']
  must_contain: ['AAPL', '150', '198.45']
  must_not_contain: ['GOOGL', 'AMZN'] # Hallucination if mentioned
  expected_verified: true
  tags: ['happy-path', 'portfolio']
```

The `must_not_contain` assertions become much more powerful — we can assert that the LLM doesn't mention any symbol that isn't in the fixture.

### Implementation: Mock HTTP Layer in Rust

The tool dispatch function (`tools/mod.rs`) currently takes a `GhostfolioClient` reference:

```rust
pub async fn dispatch(
    client: &GhostfolioClient,
    tool_name: &str,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError>
```

For eval mode, a `ToolDispatcher` trait abstracts over live and mock dispatch:

```rust
pub trait ToolDispatcher {
    async fn dispatch(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value, ApiError>;
}

/// Live dispatcher — calls real Ghostfolio API
impl ToolDispatcher for GhostfolioClient { ... }

/// Mock dispatcher — returns fixture data
struct MockDispatcher {
    fixture: FixtureSet,  // tool_name → JSON (possibly keyed by param)
}

impl ToolDispatcher for MockDispatcher {
    async fn dispatch(&self, tool_name: &str, input: &Value) -> Result<Value, ApiError> {
        // Lookup fixture, resolve parameter key if needed
    }
}
```

The agent loop takes `impl ToolDispatcher` instead of `&GhostfolioClient`. In normal TUI mode, it gets the live client. In eval mode, it gets a `MockDispatcher` loaded from the suite's fixture set.

### Generating Fixtures

Fixtures can be created in three ways:

1. **Manual curation** — Write JSON by hand for small, precise datasets. Best for edge cases.
2. **Snapshot from live server** — Run a script that calls every Ghostfolio API endpoint against a real instance and saves the responses. Best for creating the initial `moderate-portfolio` fixture.
3. **From `seed-portfolio.ts`** — The existing portfolio seeder generates realistic data. A snapshot script can capture the API responses after seeding.

A `snapshot` subcommand could automate option 2:

```bash
# Connect to running Ghostfolio, snapshot all API responses into a fixture set
cargo run -- snapshot --name moderate-portfolio --output evals/fixtures/moderate-portfolio/
```

### Updated Directory Structure

```
evals/
├── golden_sets/                        # Stage 1: Regression tests
│   ├── account.yaml
│   ├── market.yaml
│   └── ...
├── scenarios/                          # Stage 2: Coverage mapping
│   ├── multi_tool.yaml
│   ├── adversarial.yaml
│   └── ...
├── fixtures/                           # Golden data sets for mock API
│   ├── moderate-portfolio/
│   │   ├── _manifest.yaml
│   │   ├── get_portfolio_summary.json
│   │   ├── get_holdings.json
│   │   ├── get_holding_detail.json
│   │   ├── get_performance.json
│   │   ├── list_accounts.json
│   │   ├── list_activities.json
│   │   ├── get_benchmarks.json
│   │   ├── search_assets.json
│   │   ├── get_asset_profile.json
│   │   └── get_market_data.json
│   ├── empty-portfolio/
│   │   └── ...
│   └── error-states/
│       └── ...
├── rubrics/                            # Stage 4: LLM-as-judge
│   └── rubrics.yaml
├── suites.yaml                         # Suite → fixture binding
├── models.yaml
├── results/
└── runner/
```

---

## 11. Implementation Priority

### Phase 1: Evals Restructure — DONE (P0 + P1), IN PROGRESS (P2 + P3)

| Priority | Work Item                                                                      | Effort | Status      | Commit    |
| -------- | ------------------------------------------------------------------------------ | ------ | ----------- | --------- |
| **P0**   | Rename fields (`prompt`→`query`, `expected_in_response`→`must_contain`)        | Low    | **Done**    | e35fd18df |
| **P0**   | Add `must_not_contain` field + grader check                                    | Low    | **Done**    | e35fd18df |
| **P0**   | Restructure directories (`cases/`→`golden_sets/`+`scenarios/`, code→`runner/`) | Low    | **Done**    | e35fd18df |
| **P0**   | Update `suites.yaml` with `stage` field                                        | Low    | **Done**    | e35fd18df |
| **P1**   | Add `category` + `difficulty` fields to all cases                              | Low    | **Done**    | 03256a250 |
| **P1**   | Add coverage matrix to summary output                                          | Medium | **Done**    | 03256a250 |
| **P1**   | Create `rubrics/rubrics.yaml` scaffold                                         | Low    | **Done**    | 03256a250 |
| **P2**   | Implement Tier D rubric scorer (LLM-as-judge)                                  | Medium | Not started | —         |
| **P2**   | Add cost estimation + p50/p95 latency to comparison output                     | Low    | **Done**    | —         |
| **P3**   | Add `--replay` flag for re-grading historical results                          | Medium | Not started | —         |

### Phase 2: CLI Multi-Provider Support (Completed — Unblocks Phase 3)

| Priority | Work Item                                            | Effort | Status              |
| -------- | ---------------------------------------------------- | ------ | ------------------- |
| **P0**   | Extract provider-aware client dispatch (`LlmClient`) | Low    | **Done**            |
| **P0**   | Implement `OpenAIClient` with format translation     | Medium | **Done**            |
| **P0**   | Add provider selection from env/config keys          | Low    | **Done**            |
| **P1**   | Wire provider selection into `app.rs` / agent loop   | Low    | **Done**            |
| **P1**   | Update config with provider key resolution/status    | Low    | **Done**            |
| **P2**   | Add `--provider` CLI flag for explicit override      | Low    | Not started (defer) |

### Phase 3: Rust Eval Runner with Mock Layer (Complete)

| Priority | Work Item                                                                          | Effort | Status      |
| -------- | ---------------------------------------------------------------------------------- | ------ | ----------- |
| —        | Extract `ToolDispatcher` enum from `dispatch()`                                    | Low    | **Done**    |
| —        | Implement `MockDispatcher` (loads fixture set, resolves by tool + params)          | Medium | **Done**    |
| —        | Create `moderate-portfolio` fixture set (snapshot from live server or hand-curate) | Medium | **Done**    |
| —        | Create `_manifest.yaml` with ground-truth key facts for each fixture               | Low    | **Done**    |
| —        | Create `empty-portfolio` and `error-states` edge-case fixtures                     | Low    | Not started |
| —        | Add `fixture` field to `suites.yaml` (suite → fixture binding)                     | Low    | **Done**    |
| —        | Add `test` subcommand to CLI                                                       | Medium | **Done**    |
| —        | YAML case loader (read `golden_sets/`, `scenarios/`)                               | Medium | **Done**    |
| —        | Grading logic (Tiers A-C) in Rust                                                  | Medium | **Done**    |
| —        | Results output (NDJSON + SQLite)                                                   | Medium | **Done**    |
| —        | LangSmith tracing in eval runner                                                   | Low    | **Done**    |
| —        | Parallel execution with bounded concurrency                                        | Medium | **Done**    |
| —        | Multi-model matrix with comparison summaries                                       | Medium | **Done**    |
| —        | Vendor eval definitions into `cli/evals/`                                          | Low    | **Done**    |
| —        | Update `must_contain` / `must_not_contain` in cases to use exact fixture values    | Medium | Not started |
| —        | Optional: `snapshot` subcommand to capture fixtures from live server               | Low    | Not started |
| —        | Remove TS runner (once parity gates pass)                                          | Low    | Not started |
