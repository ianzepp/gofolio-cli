# AgentForge Pre-Search: Ghostfolio Finance Agent

**Project:** AgentForge Week 2 — Finance domain on Ghostfolio
**Base repo:** https://github.com/ghostfolio/ghostfolio
**Date:** 2026-02-23

---

## Phase 1: Define Your Constraints

### 1. Domain Selection

- **Domain:** Personal finance / portfolio management
- **Use cases:** Portfolio analysis and holdings summary, transaction categorization, compliance/rules checking (concentration risk, fee ratios, emergency fund), market data lookup, account overview, exchange rate conversion, benchmark comparison, performance history
- **Verification requirements:** Numerical claims (portfolio values, allocation percentages) must be cross-referenced against live Ghostfolio service data; LLM-synthesized recommendations must be flagged as non-financial-advice; hallucinated ticker symbols or asset data must be detected
- **Data sources:** Ghostfolio's own services (portfolio calculator, rules engine, 9 market data providers including Yahoo Finance, Alpha Vantage, CoinGecko); internal Prisma DB via NestJS services

### 2. Scale & Performance

- **Expected query volume:** Low — demo/prototype scale, single developer + evaluator. Target: <10 concurrent sessions.
- **Acceptable latency:** <5s for single-tool queries; <15s for 3+ tool chains (matches assignment targets)
- **Concurrent user requirements:** Not a concern for this sprint; single-user demo is sufficient
- **Cost constraints:** Keep total LLM spend under $50 for the full sprint including eval runs. Per-query target: <$0.05 at 1,000-user scale.

### 3. Reliability Requirements

- **Cost of a wrong answer:** Medium-high. Financial data errors could mislead portfolio decisions. For this demo context, the agent must clearly disclaim it is not providing financial advice and must surface data sources.
- **Non-negotiable verification:** All numerical portfolio data must come from Ghostfolio services, not LLM-generated. The LLM may only synthesize and explain — never invent values.
- **Human-in-the-loop:** Not implemented for MVP. Escalation trigger for stretch goal: if confidence score is below threshold, surface a "please verify with a financial advisor" disclaimer.
- **Audit/compliance:** Not required for demo. Agent traces logged via LangSmith for debugging and eval analysis.

### 4. Team & Skill Constraints

- **Framework familiarity:** High. Direct Anthropic tool calling already built in TypeScript (monk-api). Three full agentic systems built in Rust (abbot, prior, gauntlet-week-1). Vercel AI SDK is new but follows same patterns.
- **Domain experience:** Moderate. Familiar with portfolio concepts; not a finance professional. Ghostfolio's rules engine provides the domain expertise as code.
- **Eval/testing comfort:** High. Built ai-trials (Python) and faber-trials (TypeScript) eval harnesses. YAML task definitions, layered graders, and JSONL+SQLite dual-write are all prior art.

---

## Phase 2: Architecture Discovery

### 5. Agent Framework Selection

- **Choice:** Vercel AI SDK (already in Ghostfolio codebase)
- **Rationale:** Supports per-call model switching (`model` parameter on `generateText`/`streamText`), enabling multi-model eval comparisons without code changes. TypeScript-native. No added abstraction beyond what's needed.
- **Architecture:** Single agent, ReAct loop (iterate until LLM stops calling tools, max 10 rounds)
- **State management:** Conversation history in-memory per session, persisted to a new Prisma model (`AgentSession`) for reload on reconnect. Truncation rules from gauntlet-week-1.
- **Tool complexity:** Medium. 7-8 tools total; 5 are direct NestJS service calls (Tier 1); 2-3 go through HTTP self-calls to PortfolioService (Tier 2). All tools return structured JSON.

### 6. LLM Selection

- **Primary model:** `claude-sonnet-4-5` (Anthropic) — default for development and demos
- **Multi-model eval:** Eval runs will sweep across models (claude-sonnet-4-5, claude-opus-4-6, gpt-4o minimum) using Vercel AI SDK's per-call `model` parameter. This is a primary project output alongside the agent itself.
- **Function calling:** All target models support function calling / tool use. Verified.
- **Context window:** 200k tokens (Claude). Portfolio data responses are typically <2k tokens per tool call. No context window pressure expected.
- **Cost per query:** ~$0.01–0.03 for claude-sonnet-4-5 at typical tool chain depth (3-4 tool calls + synthesis).

### 7. Tool Design

**Planned tools (7-8 total):**

| Tool                    | Ghostfolio Source                  | Access Method  |
| ----------------------- | ---------------------------------- | -------------- |
| `portfolio_analysis`    | PortfolioService.getDetails        | HTTP self-call |
| `portfolio_performance` | PortfolioService.getPerformance    | HTTP self-call |
| `account_overview`      | AccountService.getAccounts         | Direct DI      |
| `transaction_history`   | OrderService.getOrders             | Direct DI      |
| `market_data`           | DataProviderService.getQuotes      | Direct DI      |
| `compliance_check`      | RulesService.evaluate              | Direct DI      |
| `exchange_rate`         | ExchangeRateDataService.toCurrency | Direct DI      |
| `benchmark_compare`     | BenchmarkService.getBenchmarks     | Direct DI      |

- **External API dependencies:** Yahoo Finance (no key needed), optionally Alpha Vantage or CoinGecko (free tier)
- **Mock vs real:** Development uses real Ghostfolio services with a local Docker stack (Postgres + Redis). Eval runs use the same live stack with a seeded test user. No mocking of Ghostfolio services — fixture capture is the fallback if Docker stability is a concern.
- **Error handling:** All tools return `{ ok: boolean, data?: T, error?: string }`. Errors are returned as tool results (not thrown), allowing the LLM to self-correct or explain.

### 8. Observability Strategy

- **Tool:** LangSmith (SaaS, free tier)
- **Integration:** Wrap the Vercel AI SDK calls with LangSmith tracing via `@langchain/core/tracers` or the LangSmith SDK's manual trace API
- **Key metrics:** End-to-end latency, per-tool execution time, LLM input/output tokens, cost per query, tool selection accuracy (from evals)
- **Real-time monitoring:** Not required for sprint. LangSmith dashboard provides post-hoc trace inspection.
- **Cost tracking:** LangSmith captures token counts; cost computed from model pricing config in the eval framework.

### 9. Eval Approach

- **Framework:** Custom TypeScript runner (ported from faber-trials patterns), co-located in `apps/agent-evals/`
- **Task format:** YAML files with `id`, `type`, `prompt`, `expected_tool_calls`, `expected_output`, `tags`, `judge_criteria`
- **Grading:** Three-layer — (A) correct tool selected, (B) parameters correct, (C) response quality via LLM-as-judge
- **Ground truth:** Seeded Ghostfolio test user with deterministic portfolio data (known holdings, known transactions, known balances)
- **Models swept:** claude-sonnet-4-5, claude-opus-4-6, gpt-4o — same 50+ tasks, comparative results
- **Automated:** Yes, fully automated CLI runner. Results stored JSONL + SQLite with aggregate views.
- **CI integration:** Out of scope for sprint. Eval suite run manually; results committed to repo.

### 10. Verification Design

- **Claims that must be verified:** Portfolio values and allocations (sourced from Ghostfolio, not LLM); compliance rule outcomes (pass/fail from RulesService); market prices (from DataProviderService)
- **Fact-checking sources:** All numerical data returned from tool calls is authoritative. LLM synthesis is layered on top — the agent cites the tool that produced each data point.
- **Confidence thresholds:** If a tool returns an error or empty result, the agent must say so explicitly rather than estimating. No confidence scoring on tool outputs (they are authoritative). LLM-generated interpretation is always labeled as interpretation.
- **Escalation triggers:** MVP — none. Stretch goal: surface disclaimer if LLM attempts to answer a financial question without a supporting tool call.

---

## Phase 3: Post-Stack Refinement

### 11. Failure Mode Analysis

- **Tool failure:** Return `{ ok: false, error: "..." }` as the tool result. LLM receives the error and responds to the user explaining what failed. No silent failures.
- **Ambiguous queries:** LLM asks a clarifying question (standard ReAct behavior). No special handling needed.
- **External market data unavailable:** Yahoo Finance is scraping-based and can be flaky. Fallback: return cached/stale data with a staleness warning. If no data at all, tool returns error.
- **PortfolioService HTTP self-call fails:** Retry once, then return error. The agent explains the portfolio data is temporarily unavailable.
- **Max iterations reached:** Return partial response with a note that the query required more steps than the iteration limit. Increase limit if this occurs frequently in evals.
- **Graceful degradation:** Tools are independent. Failure of one tool does not block others. Agent can partially answer using available tool results.

### 12. Security Considerations

- **Prompt injection:** User input is never interpolated directly into tool parameters. All tool inputs are extracted by the LLM from structured schemas. Wrap user messages in `<user_input>` tags in the system prompt (gauntlet-week-1 pattern).
- **Data leakage:** Portfolio data belongs to the authenticated Ghostfolio user. The agent only operates on the authenticated user's data. No cross-user data access. LangSmith traces will contain portfolio data — acceptable for demo; would require PII scrubbing in production.
- **API key management:** All keys (Anthropic, market data providers, LangSmith) stored in environment variables. No keys in code or committed to repo. Ghostfolio's existing `.env` pattern extended.
- **Audit logging:** LangSmith captures full trace per request. Sufficient for demo. No separate audit log.

### 13. Testing Strategy

- **Unit tests for tools:** Each tool function gets a Jest unit test with a mocked Ghostfolio service dependency. Tests verify correct parameter mapping and error handling.
- **Integration tests:** Eval suite serves as integration tests — 50+ cases run against live agent with real Ghostfolio services.
- **Adversarial testing:** 10+ adversarial cases in the eval suite: prompt injection attempts, requests for financial advice beyond data scope, malformed inputs, questions about other users' data.
- **Regression testing:** Eval suite baseline committed to repo. Any score drop from baseline is a regression signal. Not automated (manual run before submission).

### 14. Open Source Contribution

- **What:** The 50+ eval test cases, published as an npm package `ghostfolio-agent-evals`
- **License:** MIT (agent eval layer is independent of the AGPL Ghostfolio codebase)
- **Contents:** YAML task definitions, TypeScript runner, SQLite schema with aggregate views, README with usage instructions
- **Documentation:** README covers installation, running evals, interpreting results, adding new test cases
- **Community:** Published to npm. Linked in the Ghostfolio GitHub Discussions if appropriate. No active maintenance commitment beyond the sprint.

### 15. Deployment & Operations

- **Hosting:** Single VPS (DigitalOcean, 2GB RAM, $12/mo). Docker Compose: Postgres + Redis + Ghostfolio app (which serves both API and Angular client) + agent NestJS module (co-located in the same app).
- **SSL:** Caddy reverse proxy for automatic HTTPS (Let's Encrypt). Single additional service.
- **CI/CD:** None for sprint. Manual deploy via `docker compose up --build` on the VPS.
- **Monitoring:** LangSmith for agent traces. No infrastructure alerting for demo.
- **Rollback:** `git revert` + `docker compose up --build`. Acceptable for demo context.

### 16. Iteration Planning

- **User feedback:** Thumbs up/down on agent responses (stored to Prisma `AgentFeedback` table). Surfaced in LangSmith via metadata tags.
- **Eval-driven improvement:** After each significant change, run the full eval suite and compare scores against the committed baseline. Failures drive prompt updates or tool fixes.
- **Feature prioritization:** Priority order matches assignment build strategy — tools first, then observability, then evals, then verification layer.
- **Maintenance:** Ghostfolio upstream is actively maintained (~3 commits/day). Merge upstream changes periodically. Agent module is isolated enough that upstream merges should not break it.

---

## Decision Summary

| Decision         | Choice                                     | Rationale                                                              |
| ---------------- | ------------------------------------------ | ---------------------------------------------------------------------- |
| Domain           | Finance (Ghostfolio)                       | Rich wrappable domain logic, TypeScript stack, clean Docker deployment |
| Agent framework  | Vercel AI SDK                              | Already in codebase, per-call model switching for multi-model evals    |
| Primary LLM      | claude-sonnet-4-5                          | Best capability/cost balance; prior experience                         |
| Eval LLM sweep   | claude-sonnet-4-5, claude-opus-4-6, gpt-4o | Multi-model comparison is a project output                             |
| Observability    | LangSmith                                  | SaaS free tier, no added Docker services, full trace capture           |
| OSS contribution | Eval dataset (npm)                         | MIT-licensable, AGPL-safe, genuinely reusable                          |
| Architecture     | Single agent, ReAct loop, max 10 rounds    | Proven pattern from gauntlet-week-1                                    |
| Tool count       | 8 tools                                    | 6 direct DI (Day 1), 2 HTTP self-call (Day 2)                          |
| Deployment       | VPS + Docker Compose + Caddy               | Simplest path to publicly accessible demo                              |
