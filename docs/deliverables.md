# Deliverables Checklist

**Deadline:** Sunday Feb 28, 10:59 PM CT
**Last updated:** Fri Feb 27, 2026 (evidence-based audit, CLI verification + replay/report updates)

---

## MVP Gate (24-Hour — Tuesday)

- [x] Agent responds to natural language queries in finance domain
- [x] At least 3 functional tools (`apps/api`: 11 tools, `cli`: 18 tools)
- [x] Tool calls execute successfully and return structured results
- [x] Agent synthesizes tool results into coherent responses
- [x] Conversation history maintained across turns
- [x] Basic error handling (graceful failure, not crashes)
- [x] At least one domain-specific verification check
- [x] Simple evaluation: 5+ test cases with expected outcomes (`cli/evals`: 73 cases)
- [~] Deployed and publicly accessible (URL is documented; live endpoint not re-verified in this code audit)

## Core Agent (5+ tools, verification, eval)

- [x] 5+ functional tools
- [x] ReAct loop with multi-step reasoning (`apps/api` uses `maxSteps`, `cli` uses 20 tool rounds)
- [x] Multi-turn conversation (`apps/api` in-memory session service; CLI conversation state)
- [~] Verification layer with 3+ distinct checks (CLI now has 2 deterministic checks: `claim_to_tool_grounding` + `tool_error_propagation`, plus optional LLM `secondary_review` when `GF_VERIFY_PROVIDER` or `GF_VERIFY_MODEL` is configured)
- [x] Tool-first system prompt workflow in both surfaces

## Evaluation Framework (50+ cases)

- [x] Eval harness with grading (Tiers A/B/C) in `cli/src/evals.rs`
- [x] 50+ test cases (73 total across `golden_sets` + `scenarios`)
- [x] Dataset mix meets minima (20+ happy path, 10+ edge, 10+ adversarial, 10+ multi-step)
- [x] Multi-model sweep support (`cli/evals/models.yaml` has 34 models)
- [x] Coverage matrix reporting (`ghostfolio evals report` prints category/difficulty breakdown and category × difficulty matrix)
- [~] Cost estimation + latency reporting in eval output (cost estimation implemented in CLI status UI; p50/p95 percentile reporting is not implemented, current latency summary surfaces current/avg/max in seconds)
- [~] Rubric scorer (Tier D, LLM-as-judge): rubric config exists, scorer not implemented
- [x] Replay command for re-grading historical results (`ghostfolio evals replay --run-id <id>` or `--path <run-dir>`)

## Observability

- [x] Trace logging (request/step/tool level events)
- [x] Latency tracking (request + step + tool timings)
- [x] Error tracking (`ok/error` tool results and structured error paths)
- [x] Token usage tracking (input/output and per-step)
- [x] Eval results persistence (SQLite + per-case JSON files)
- [x] LangSmith integration (API + CLI code paths present)
- [~] User feedback mechanism (API endpoint + CLI thumbs controls exist; client UI flow/persistence is incomplete)

## Submission Deliverables

- [x] GitHub repository with setup guide + architecture overview
- [x] Pre-search document (`cli/docs/presearch.md`)
- [~] Deployed application evidence documented (URL present; not runtime-verified during this pass)
- [~] Open source contribution partial (work exists, but `gauntlet/package.json` is `private: true`; no published package/PR/dataset evidence in repo)
- [ ] Agent architecture doc (1-2 pages)
- [ ] AI cost analysis (dev spend + projections for 100/1K/10K/100K users)
- [ ] Demo video (3-5 min)
- [ ] Social post (X or LinkedIn, tag @GauntletAI)

## Notes

- This checklist intentionally ignores previous done/not-done claims and reflects current repository evidence.
- Eval artifact format has migrated to per-case JSON files under `cli/evals/results/<run-id>/` plus SQLite.
