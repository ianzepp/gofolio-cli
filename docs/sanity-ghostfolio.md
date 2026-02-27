# Sanity Check: Ghostfolio as AgentForge Base Project

## 1. Project Identity

| Attribute            | Value                                    |
| -------------------- | ---------------------------------------- |
| **Name**             | Ghostfolio                               |
| **Domain**           | Personal finance / portfolio tracking    |
| **Repository**       | https://github.com/ghostfolio/ghostfolio |
| **License**          | AGPL v3 (strict copyleft)                |
| **Current Version**  | v2.242.0 (released Feb 22, 2026)         |
| **Commit Count**     | ~4,660                                   |
| **Recent Velocity**  | ~187 commits in last 2 months (~3/day)   |
| **Primary Language** | TypeScript (100% of app code)            |

---

## 2. Technology Stack

| Layer                       | Technology               | Version                 |
| --------------------------- | ------------------------ | ----------------------- |
| **Backend framework**       | NestJS                   | 11.1.8                  |
| **Frontend framework**      | Angular                  | 21.1.1                  |
| **Build system / monorepo** | Nx                       | 22.4.5                  |
| **ORM**                     | Prisma                   | 6.19.0                  |
| **Database**                | PostgreSQL               | 15                      |
| **Cache / queue**           | Redis + Bull             | Redis Alpine / Bull 4.x |
| **Runtime**                 | Node.js                  | >= 22.18.0              |
| **TypeScript**              | TypeScript               | 5.9.2                   |
| **Test framework**          | Jest                     | 30.2.0                  |
| **UI library**              | Angular Material + Ionic | 21.1.1 / 8.7.8          |
| **Package manager**         | npm                      | v10+                    |

---

## 3. Codebase Size and Shape

| Metric                         | Count   |
| ------------------------------ | ------- |
| TypeScript files               | ~752    |
| HTML templates                 | ~150    |
| SCSS stylesheets               | ~127    |
| Total estimated TypeScript LOC | ~55,000 |
| Angular components             | ~110    |
| NestJS controllers             | 33      |
| REST API endpoints             | ~146    |
| NestJS service classes         | ~32     |
| Prisma models                  | 19      |
| Prisma enums                   | 9       |
| Database migrations            | 107     |
| Test spec files                | 30      |
| Production dependencies        | 82      |
| Dev dependencies               | 64      |

### Monorepo Structure

```
ghostfolio/
├── apps/
│   ├── api/          # NestJS backend (~37,000 LOC)
│   └── client/       # Angular frontend (~18,000 LOC)
├── libs/
│   ├── common/       # Shared types, helpers, config (163 .ts files)
│   └── ui/           # Shared Angular components (125 .ts files)
├── prisma/           # Schema + 107 migrations
├── docker/           # Docker Compose (dev + prod)
└── ...
```

---

## 4. Existing AI Integration

Ghostfolio has a **minimal, experimental AI feature** that was recently added:

- **Files**: `ai.controller.ts`, `ai.service.ts`, `ai.module.ts`
- **What it does**: Generates a markdown table of user holdings and wraps it in a structured prompt. The user can copy-paste this into an external LLM (Duck.ai, ChatGPT, etc.) or the app can call OpenRouter directly via the Vercel `ai` SDK.
- **Prompt modes**: `portfolio` (raw data) and `analysis` (structured analysis request with predefined sections like Risk Assessment, Advantages, Disadvantages, etc.)
- **What it does NOT do**:
  - No tool calling / function calling
  - No multi-turn conversation
  - No memory or session persistence
  - No agent loop or orchestration
  - No structured output parsing
  - No verification of AI responses
  - No observability or token tracking

**Bottom line**: This is a prompt exporter, not an agent. It provides zero scaffolding for the AgentForge requirements. Everything would need to be built from scratch.

---

## 5. Domain Logic Available for Agent Tool Wrapping

This is Ghostfolio's strongest asset for the AgentForge project. There is rich, mature business logic that could be exposed as agent tools:

### Portfolio Calculation Engine (HIGH value, HIGH complexity)

- 4 calculation strategies via factory pattern: ROAI, TWR, MWR, ROI
- Abstract base class: ~1,009 lines of financial math
- Computes: performance, allocation, dividends, investments over time
- **Red flag**: Deeply coupled to Ghostfolio data models. Requires adapter layer to expose as tool. Not a simple wrapper.

### Rules / Compliance Engine (HIGH value, MEDIUM complexity)

- 18 rule implementations covering:
  - Account cluster risk (concentration in single account)
  - Asset class cluster risk (equity vs fixed income balance)
  - Currency cluster risk (exposure to non-base currencies)
  - Regional/economic market risk (geographic diversification)
  - Fee ratio analysis
  - Emergency fund verification
  - Buying power / liquidity checks
- Each rule: configurable thresholds, localized output, boolean pass/fail
- **This maps directly to AgentForge's "verification" and "compliance_check" requirements**

### Market Data Providers (MEDIUM value, LOW complexity)

- 9 integrated data sources: Yahoo Finance, Alpha Vantage, CoinGecko, EOD Historical Data, Financial Modeling Prep, RapidAPI, Google Sheets, Manual, Ghostfolio
- 3 data enhancers: TrackInsight, OpenFIGI, Yahoo Finance
- Well-abstracted `DataProviderInterface` with standard methods
- Redis caching layer on top
- **Could be wrapped as `market_data()` tool relatively easily**

### Transaction / Order Management (MEDIUM value, LOW complexity)

- Full CRUD for activities: BUY, SELL, DIVIDEND, FEE, INTEREST, LIABILITY
- Filtering by date, account, asset class, symbol, tags
- Draft support
- **Could be wrapped as `transaction_categorize()` or `transaction_history()` tools**

### Account & Balance Management (MEDIUM value, LOW complexity)

- Multi-account support with platform association
- Historical balance snapshots
- Exclusion logic (exclude accounts from analysis)

### Exchange Rate Service (LOW value, LOW complexity)

- Currency conversion between any supported currencies
- Historical rates
- **Trivial to wrap as a tool**

### Import/Export (LOW value, LOW complexity)

- CSV import with platform-specific parsers
- JSON export for backup
- Google Sheets integration

---

## 6. AgentForge Requirements Gap Analysis

### MVP Requirements (24-hour gate)

| Requirement                                         | Current State                                 | Effort to Build                                       | Risk   |
| --------------------------------------------------- | --------------------------------------------- | ----------------------------------------------------- | ------ |
| Agent responds to NL queries                        | Prompt exporter only, no conversational agent | HIGH - need full agent framework integration          | Medium |
| At least 3 functional tools (MVP); 5 minimum (full) | Zero tools defined                            | MEDIUM - domain logic exists to wrap                  | Low    |
| Tool calls execute with structured results          | No function calling                           | HIGH - need tool schema definitions + execution layer | Medium |
| Agent synthesizes tool results                      | No synthesis                                  | MEDIUM - comes with agent framework                   | Low    |
| Conversation history across turns                   | No memory system                              | MEDIUM - need session/memory layer                    | Low    |
| Basic error handling                                | No agent error handling                       | LOW - standard practice                               | Low    |
| Domain-specific verification check                  | Rules engine exists but not connected to AI   | MEDIUM - need to bridge rules engine to agent         | Low    |
| 5+ test cases with expected outcomes                | No AI-specific tests                          | LOW - manual creation                                 | Low    |
| Deployed and publicly accessible                    | Docker deployment exists                      | LOW - infrastructure is ready                         | Low    |

### Full Requirements (5+ tools, eval, observability, verification)

| Requirement                             | Effort     | Notes                                                                  |
| --------------------------------------- | ---------- | ---------------------------------------------------------------------- |
| 5+ agent tools                          | MEDIUM     | Domain logic exists; wrapping it requires understanding deep internals |
| 50+ eval test cases                     | MEDIUM     | Must be created from scratch; no existing AI test infrastructure       |
| Observability (traces, tokens, latency) | MEDIUM     | No existing AI observability; need to add from scratch                 |
| 3+ verification checks                  | LOW-MEDIUM | Rules engine provides foundation; need to connect to agent output      |
| Performance targets (<5s latency)       | UNKNOWN    | Depends on LLM provider and tool chain depth                           |
| Cost analysis                           | LOW        | Straightforward once agent is running                                  |

---

## 7. Red Flags and Pitfalls

### LICENSE: AGPL v3 (CRITICAL to understand)

The AGPL v3 is the strictest common open-source license. If you deploy a modified Ghostfolio as a **network service** (which is exactly what the AgentForge project requires — "deployed and publicly accessible"), you **must** make your complete modified source code available to all users under AGPL v3. This includes your agent code, tools, eval framework, and everything else in the repo.

- **For this classroom project**: Probably fine since you're submitting source code anyway.
- **For any future commercial use**: This is a hard blocker. You cannot build proprietary features on top of AGPL v3 code and serve them over a network.
- **For the open source contribution requirement**: Actually helps — AGPL already requires openness.

### LANGUAGE: TypeScript-only (double-edged sword)

The entire stack is TypeScript. The AgentForge doc recommends Python/FastAPI with LangChain/LangGraph, which have the most mature agent framework ecosystems. Building in TypeScript means:

- **LangChain.js exists** but is less mature than Python LangChain. Fewer examples, fewer community tools, fewer tutorials.
- **LangGraph.js exists** but is even less mature.
- **Vercel AI SDK** is already in the project and is TypeScript-native, but it's more of a streaming/generation library than a full agent framework.
- **No Python escape hatch**: You can't easily mix Python agent frameworks into a NestJS monorepo. You'd either need to build a separate Python microservice or commit fully to the JS agent ecosystem.
- **Upside**: If you're comfortable with TypeScript, you avoid context-switching between languages. One language for everything.

### FRAMEWORK COMPLEXITY: NestJS + Angular + Nx

This is a **full-stack enterprise monorepo**. The learning curve is steep if you're not already familiar with similar patterns:

- **NestJS**: Decorators, modules, dependency injection, guards, interceptors, pipes. Not a simple Express app.
- **Angular**: Component lifecycle, observables/RxJS, modules, dependency injection (different from NestJS DI), Material Design, forms.
- **Nx**: Workspace configuration, task runners, affected builds, library boundaries.
- **Prisma**: Schema language, migrations, client generation, relation management.

**Time to productivity estimate (generic)**: If you don't know NestJS, budget 1-2 days just to understand the module system, DI, and how controllers/services/guards interact. If you don't know Angular, budget another 1-2 days for the frontend. That's potentially half the project week gone before writing agent code.

**Time to productivity estimate (with existing TS expertise)**: With prior experience building a 59k-line strict-mode TypeScript backend (monk-api) using Hono, direct Anthropic API integration, tool use, DI-like patterns, and decorators — NestJS is a more opinionated version of the same patterns. Budget half a day for NestJS orientation (module system, guard decorators, request scoping), not 1-2 days. Angular remains a separate learning curve if a frontend chat UI is needed, but the backend agent work does not require Angular knowledge.

### DATABASE: 107 Migrations, 19 Models

The schema is mature and complex. Adding new models (e.g., for conversation history, agent sessions, eval results) requires:

1. Modifying `schema.prisma`
2. Creating a migration (`npx prisma migrate dev`)
3. Regenerating the Prisma client
4. Building new services, controllers, DTOs
5. Testing the migration path

This isn't hard per se, but it adds friction to every schema change. And with 19 existing models and extensive relationships, you need to understand the data model before touching it.

### TEST COVERAGE: Sparse (30 spec files for 752 TS files)

Only ~4% of files have corresponding tests. The existing tests are concentrated in:

- Portfolio calculator (financial math correctness)
- Common library helpers

There are **no tests for most controllers or services**. This means:

- You have no safety net when modifying existing code
- You can't rely on existing tests to catch regressions from your changes
- The eval framework you build for AgentForge will be the most tested part of the codebase

### TYPESCRIPT STRICTNESS: Disabled

```json
"strict": false
"strictNullChecks": false
"strictPropertyInitialization": false
```

The codebase runs with TypeScript strict mode **off**. This means:

- Null/undefined errors are not caught at compile time
- Property initialization is not enforced
- You may encounter runtime errors that strict mode would have prevented
- Your agent code could introduce subtle bugs that the compiler won't flag

### NODE VERSION: >= 22.18.0

This is a very recent Node requirement. Local machine confirmed at v24.6.0, so no issue for development. Most cloud providers and the project's own Dockerfile (node:22-slim) handle this. Not a practical concern.

### EXTERNAL DEPENDENCIES FOR FULL FUNCTIONALITY

To get meaningful financial data flowing through the system (which your agent tools would need), you need at least one market data provider configured:

- **Yahoo Finance**: Works without API key (scraping-based), but can be flaky
- **CoinGecko**: Free tier available with demo key
- **Alpha Vantage**: Free tier with 5 calls/minute limit
- Others require paid API keys

Without market data, your portfolio analysis tools return stale or empty results.

### PREMIUM/SUBSCRIPTION SYSTEM (Stripe)

Ghostfolio has a premium tier with Stripe integration. Some features are gated behind subscriptions:

- The Ghostfolio data source (community-sourced data)
- Some rules engine features (X-Ray analysis)
- Certain UI features

For the AgentForge project, you'd need to either:

1. Ignore premium features (limit what your agent can access)
2. Disable the subscription gate (modify authorization logic)
3. Leave it as-is and document the limitation

### HARDCODED REFERENCES

12+ hardcoded references to `ghostfol.io` throughout the codebase (support emails, API endpoints, pricing pages). These are cosmetic but unprofessional if left in a deployed fork.

### NO PLUGIN/EXTENSION ARCHITECTURE

Ghostfolio was not designed to be extended by third parties. There is no plugin system, no hook registry, no extension points. Adding the agent system means directly modifying core application code — new NestJS modules imported into the main app module, new routes added to existing controllers or new controllers registered, etc.

---

## 8. Agent Tool Integration Surface Area

This section answers the critical question: **how hard is it for agent tools to reach into Ghostfolio and get useful data back?** This is independent of the LLM/agent framework complexity — it's purely about the coupling between your tool functions and Ghostfolio's internals.

### The Architecture Constraint: Request-Scoped Services

NestJS services can be **singleton** (one instance shared across all requests) or **request-scoped** (new instance per HTTP request, with the authenticated user injected). The distinction matters because request-scoped services cannot be called from outside an HTTP request context — such as from an agent tool running in a background process or a different module.

23 files in the API use `@Inject(REQUEST)` to access the current HTTP user. Most of these are **controllers** (which is normal). But critically, `PortfolioService` — the highest-value service for agent tools — is one of them.

### What `@Inject(REQUEST)` Actually Is

This is NestJS's dependency injection wrapping the Node/Express `req` object. The chain works like this:

1. HTTP request arrives at Express
2. Passport JWT middleware validates the token and attaches the authenticated user to `req.user`
3. NestJS guards (`AuthGuard('jwt')`, `HasPermissionGuard`) run, verifying permissions
4. NestJS wraps the Express `req` as its `REQUEST` injection token
5. Any service with `@Inject(REQUEST)` receives it, typed as `RequestWithUser`

So `this.request` in PortfolioService is the Express `req` object, but `.user` is **not** just a decoded JWT payload — it's a fully hydrated Ghostfolio user object with settings, permissions, subscription status, base currency, language preference, etc. The `RequestWithUser` type is a Ghostfolio-specific type that extends the Express request.

When a service uses `@Inject(REQUEST)`, NestJS creates a **new instance of that service for every incoming HTTP request** and injects the request into the constructor. The service then reads `this.request.user` like a class-level property anywhere in its methods. This is functionally equivalent to a service that stores `req.user` as `this.currentUser` in the constructor — except the DI framework enforces that it can only exist inside a request lifecycle.

### Can `RequestWithUser` Be Mocked?

Yes, trivially — it's just an object. NestJS's testing module lets you override any provider including `REQUEST`:

```typescript
const module = await Test.createTestingModule({
  providers: [
    PortfolioService,
    {
      provide: REQUEST,
      useValue: {
        user: {
          id: 'some-user-id',
          settings: { settings: { baseCurrency: 'USD', language: 'en-US' } },
          subscription: { type: 'Premium' },
          permissions: ['readAiPrompt', '...']
        }
      }
    }
  ]
}).compile();
```

This means Option B (described further below under "Three Options for Reaching PortfolioService") is more viable than it first appears. An agent module could look up the real user from the database via UserService (a simple singleton), construct a fake `RequestWithUser` with that user's data, and use NestJS's `ModuleRef` to resolve a request-scoped PortfolioService instance with that fake request injected.

The risk is keeping the mock shape in sync with whatever fields PortfolioService actually reads off `this.request.user`. If upstream adds a new field access, the mock silently returns `undefined` (remember — strict mode is off, so no compile-time catch).

### How Existing Tests Handle User Injection (They Don't)

**There is zero existing precedent in this codebase for mocking user context through the NestJS DI system.** The project's 30 spec files use three patterns, none of which involve `@Inject(REQUEST)`:

**Pattern 1: Pass `null` for all constructor dependencies.** The most common approach. Services are instantiated directly, bypassing NestJS DI entirely:

```typescript
// From benchmark.service.spec.ts
benchmarkService = new BenchmarkService(null, null, null, null, null, null);

// From current-rate.service.spec.ts
currentRateService = new CurrentRateService(null, null, null, null);
```

This only works because TypeScript strict mode is off — `null` is silently accepted for any typed parameter. Tests then call only the methods that don't touch the nulled-out dependencies.

**Pattern 2: `jest.mock()` module-level replacements.** Services like `MarketDataService`, `ExchangeRateDataService`, and `CurrentRateService` are replaced at the module level with hardcoded return values. No real DI container involved.

**Pattern 3: Minimal dummy data.** The shared `userDummyData` is:

```typescript
export const userDummyData = {
  id: 'xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx'
};
```

No settings, no permissions, no subscription, no baseCurrency. It's only used as a userId string passed to the calculator factory — never as a mock request user.

**No test uses `Test.createTestingModule()`.** No test provides a mock `REQUEST`. No test exercises any controller or any request-scoped service. The only test that touches user-like data is `has-permission.guard.spec.ts`, which creates a mock execution context with `{ user: { permissions: [...] } }` — but that tests the guard in isolation, not user injection through the DI system.

**What this means for the AgentForge project**: Mocking `RequestWithUser` for agent tools is absolutely doable (NestJS docs cover it well), but you'd be the first to do it in this codebase. There is no existing pattern to copy from. You're writing the precedent.

### Tier 1: Services Callable Directly (no HTTP context needed)

These services are singletons. Your agent tool injects them via NestJS DI and calls methods with simple arguments (userId strings, symbol strings, dates). No adapter layer, no HTTP workaround.

| Service                     | Constructor Deps | Typical Call Signature                           | Input Types                       |
| --------------------------- | ---------------- | ------------------------------------------------ | --------------------------------- |
| **AccountService**          | 4                | `getAccounts(userId)`                            | string                            |
| **OrderService**            | 8                | `getOrders({ userId, userCurrency, ... })`       | strings + optional filter objects |
| **SymbolProfileService**    | 1 (Prisma only)  | `getSymbolProfiles([{ dataSource, symbol }])`    | array of 2-field objects          |
| **ExchangeRateDataService** | 4                | `toCurrency(100, 'EUR', 'USD')`                  | number + two strings              |
| **DataProviderService**     | 6                | `getQuotes({ items: [{ dataSource, symbol }] })` | array of 2-field objects          |
| **RulesService**            | 0 (no deps)      | `evaluate(rules, userSettings)`                  | pre-built rule objects + settings |
| **BenchmarkService**        | ~3               | `getBenchmarks()`                                | none                              |

These cover: account listings, transaction history, symbol/asset lookups, currency conversion, live market quotes, benchmark data, and compliance rule evaluation.

**Effort to wrap 5 tools from this tier: 1-2 days.** Data flows out cleanly with minimal transformation.

### Tier 2: PortfolioService (request-scoped — requires workaround)

`PortfolioService` is the crown jewel. It computes portfolio details, holdings with current values, performance over time, dividends, and the X-Ray compliance report. It has **13 constructor dependencies** including:

```typescript
@Inject(REQUEST) private readonly request: RequestWithUser
```

The service reads `this.request.user` internally to access:

- `this.request.user.id` — userId
- `this.request.user.settings.settings.baseCurrency` — user's currency
- `this.request.user.settings.settings.language` — for localized output
- `this.request.user.subscription.type` — premium feature gating
- `this.request.user.permissions` — authorization checks

**You cannot inject PortfolioService into an agent module and call it directly.** NestJS will not have request context to provide.

### Three Options for Reaching PortfolioService

**Option A: Internal HTTP calls (zero code changes to Ghostfolio)**

Your agent tools make HTTP requests to Ghostfolio's own REST API (`GET /api/v1/portfolio/details`, etc.). The existing controllers already handle user context setup, filter parsing, permission checks, and response formatting. Your tool just needs a valid JWT or API key for the target user.

- Proven call paths — every controller endpoint is battle-tested
- Zero modifications to existing Ghostfolio code
- Permission and subscription checks happen automatically
- Downside: HTTP round-trip overhead per tool call (~10-50ms locally)

**Option B: Replicate the controller's thin orchestration layer**

The controllers are thin. They extract `request.user`, call `impersonationService.validateImpersonationId()`, call `apiService.buildFiltersFromQueryParams()`, then pass `userId` + `filters` to the service. Your agent tool could replicate this setup if you can construct or obtain a `RequestWithUser` object.

- Avoids HTTP overhead
- Still requires understanding the exact controller transformation patterns
- Fragile — if controllers change upstream, your replication breaks

**Option C: Refactor PortfolioService to accept explicit user context**

Remove `@Inject(REQUEST)` from PortfolioService and pass user context (userId, baseCurrency, language, permissions) explicitly through method parameters instead of reading `this.request`.

- Cleanest long-term solution, also improves testability
- Requires modifying a 1,000+ line core service
- Risk of regressions with no test safety net (sparse coverage)
- Upstream merge conflict guaranteed

### Recommended Approach for 7-Day Sprint

**Use Option A (HTTP self-calls) for portfolio-related tools. Use direct DI injection (Tier 1) for everything else. Touch zero existing Ghostfolio code.**

This gives you access to all the domain logic through two clean interfaces:

1. Direct service injection for AccountService, OrderService, SymbolProfileService, ExchangeRateDataService, DataProviderService, RulesService
2. Authenticated HTTP calls for PortfolioService endpoints (details, holdings, performance, report)

The only prerequisite is a way to authenticate your agent's requests — JWT tokens and API keys are both already supported by Ghostfolio's auth system.

### What This Means for Tool Count

Mapping to AgentForge's suggested finance tools:

| AgentForge Tool            | Ghostfolio Service                     | Access Method           | Wrapping Difficulty                         |
| -------------------------- | -------------------------------------- | ----------------------- | ------------------------------------------- |
| `portfolio_analysis()`     | PortfolioService.getDetails/getReport  | HTTP call               | Medium — rich response, needs summarization |
| `transaction_categorize()` | OrderService.getOrders                 | Direct DI               | Low — query + filter                        |
| `market_data()`            | DataProviderService.getQuotes          | Direct DI               | Low — pass symbols, get prices              |
| `compliance_check()`       | RulesService.evaluate                  | Direct DI               | Medium — need to construct Rule objects     |
| `tax_estimate()`           | No existing service                    | Must build from scratch | High — no foundation exists                 |
| `account_overview()`       | AccountService.getAccounts             | Direct DI               | Low — simple query                          |
| `exchange_rate()`          | ExchangeRateDataService.toCurrency     | Direct DI               | Trivial — three primitives in, number out   |
| `symbol_lookup()`          | SymbolProfileService.getSymbolProfiles | Direct DI               | Low — pass identifiers, get profiles        |
| `benchmark_compare()`      | BenchmarkService.getBenchmarks         | Direct DI               | Low — no parameters needed                  |
| `performance_history()`    | PortfolioService.getPerformance        | HTTP call               | Medium — date range handling                |

**5 easy tools are available on day one. 7-8 are realistic within the sprint. Only `tax_estimate()` has no existing foundation.**

---

## 9. What Works in Your Favor

### Rich Domain Logic

The portfolio calculation engine, rules engine, market data abstraction, and transaction management are all mature and well-structured. These map directly to the AgentForge tool requirements (`portfolio_analysis`, `compliance_check`, `market_data`, `transaction_categorize`). You wouldn't be building financial logic from scratch — you'd be wrapping it.

### Docker-Ready Deployment

The Docker setup is production-grade with health checks, security hardening, and multi-service orchestration. The "deployed and publicly accessible" requirement is straightforward — `docker compose up` on any VPS gets you running.

### NestJS Module System

Despite the learning curve, NestJS modules are a natural fit for adding an agent system. You can create an `AgentModule` with its own controller, service, and tools, then import it into the app module. The DI system makes it easy to inject singleton services (OrderService, AccountService, DataProviderService, etc.) into your agent tools. PortfolioService requires the HTTP workaround described in Section 8.

### Existing Auth Infrastructure

JWT, API keys, role-based access — all already built. Your agent endpoints can reuse existing guards and permission checks. No need to build auth from scratch.

### Event-Driven Architecture

NestJS EventEmitter2 is already integrated. Portfolio changes emit events. This could be useful for agent observability (subscribe to events, log traces).

### Active Maintenance

The project is actively maintained with clean commit hygiene. Documentation exists. The codebase isn't abandoned or rotting.

---

## 10. Effort Estimation for AgentForge Deliverables

Estimates below are calibrated to the developer profile described in Section 15 (experienced TS developer with prior agentic implementations).

| Deliverable                                                     | Estimated Effort | Confidence  | Notes                                                                    |
| --------------------------------------------------------------- | ---------------- | ----------- | ------------------------------------------------------------------------ |
| Local dev environment setup                                     | 0.5-1 hours      | High        | Docker, env vars, seed data (see Section 14)                             |
| Understanding codebase architecture                             | 4-8 hours        | High        | NestJS patterns are familiar; focus on service signatures and data model |
| Agent framework integration (Vercel AI SDK or direct Anthropic) | 4-8 hours        | High        | Already built this in monk-api; port patterns to NestJS module           |
| 5 agent tools wrapping existing services                        | 8-16 hours       | Medium      | Tier 1 services are trivial; PortfolioService needs HTTP workaround      |
| Conversation memory / session management                        | 4-8 hours        | High        | New Prisma model + service; truncation pattern from gauntlet-week-1      |
| Verification layer (3+ checks)                                  | 4-8 hours        | High        | Rules engine provides foundation                                         |
| Eval framework (50+ test cases)                                 | 6-12 hours       | Medium-High | Reference patterns from ai-trials + faber-trials (see Section 13)        |
| Observability (traces, tokens, latency)                         | 4-8 hours        | High        | Trace pattern from gauntlet-week-1 ports directly                        |
| Frontend chat interface                                         | 4-8 hours        | Medium      | Minimal REST endpoint + simple page; skip complex Angular if needed      |
| Deployment (public)                                             | 1-2 hours        | High        | Docker setup exists (see Section 14)                                     |
| Documentation + cost analysis                                   | 4-8 hours        | High        | Straightforward                                                          |
| **TOTAL**                                                       | **~43-86 hours** |             | **For 1 person in 7 days (~112 waking hours) — comfortable margin**      |

---

## 11. Critical Decision Factors vs. Alternative (OpenEMR)

These are the factors that should drive the choice between Ghostfolio and OpenEMR:

### Choose Ghostfolio IF:

- You are comfortable with TypeScript and want a single-language stack
- You know (or can quickly learn) NestJS and Angular
- You're comfortable with a less mature JS agent framework ecosystem (LangChain.js, Vercel AI SDK) — or have already built direct LLM tool calling in TypeScript
- The finance domain interests you more than healthcare
- You want a cleaner, more modern codebase to work with
- You're comfortable with AGPL v3 obligations

### Be cautious about Ghostfolio IF:

- You'd prefer Python for the agent layer (LangChain/LangGraph are far more mature in Python)
- You don't know TypeScript or NestJS-style patterns — the learning curve eats into the 7-day timeline
- You need a large existing test suite to build confidence — Ghostfolio has almost none
- You want extension points or plugin architecture — Ghostfolio has none
- The portfolio calculation engine's complexity intimidates you — it's 1,000+ lines of financial math

### Neutral factors:

- Both projects require Docker + PostgreSQL
- Both have AGPL-compatible licenses (check OpenEMR specifically)
- Both have mature domain logic to wrap as tools
- Both require building the entire agent layer from scratch

---

## 12. Reference Implementations: Existing Agentic Projects

Three prior projects provide reference material for how to implement an agentic system. All three are custom Rust implementations by the same author — none use LangChain, LangGraph, or any off-the-shelf agent framework. They share a common lineage (each built on patterns from the previous) but vary significantly in scope and sophistication.

### 12a. Abbot (Good reference — full multi-agent system)

**What it is**: A custom Rust-based multi-agent runtime with a microkernel architecture. Agents communicate via a universal `Frame` protocol over async channels, with a kernel dispatcher routing tool calls to subsystems.

| Attribute       | Value                                                                                        |
| --------------- | -------------------------------------------------------------------------------------------- |
| Language        | Rust (tokio async, edition 2024)                                                             |
| LOC             | ~39,000 (daemon/src/)                                                                        |
| Agent framework | Custom — frame-based microkernel                                                             |
| Tools           | 33 tools across 18 syscall namespaces, defined as JSON schema + Rust trait impl              |
| Agent types     | 5 (Head, Hand, Mind, Room agents, Mind loop)                                                 |
| LLM providers   | OpenAI-compatible + Anthropic, unified abstraction                                           |
| Agent loop      | ReAct-style with parallel multi-agent rounds, transcript sync between rounds                 |
| Memory          | Private per-agent history + shared transcript; SQLite persistence (WAL mode)                 |
| Observability   | 3 layers: frame audit log (SQLite), structured tracing (61 trace points), kernel tap (debug) |
| Token tracking  | Per-interaction input/output tokens stored in `llm_interaction` table                        |

**Key patterns relevant to Ghostfolio agent**:

- **Tool definition**: JSON schema file + implementation trait. Clean separation of schema (what the LLM sees) from execution (what happens). Directly portable to TypeScript — define tool schemas as objects, implement execution as async functions.
- **Tool dispatch**: Syscall name → dispatcher → subsystem handler. Maps well to NestJS: tool name → switch/map → service method call.
- **Tool results as messages**: `{ role: "tool", tool_call_id: "...", content: "JSON result" }` — standard OpenAI/Anthropic function calling format. Same format regardless of framework.
- **Round limits**: Max iterations (20) to prevent runaway LLM loops and cost explosions. Essential for production.
- **Actor-based authorization**: Different agent roles get different tool catalogs. Could map to Ghostfolio's role system (ADMIN vs USER get different tools).
- **Error handling**: All tool calls return `{ ok: true/false, data/error }` — LLM sees errors as tool results, can retry or explain. Not a crash.

**What's NOT transferable**: Multi-agent rooms, lane-based concurrency, VFS sandboxing, frame protocol. These are Abbot-specific and far beyond what AgentForge requires.

### 12b. Prior (Good to very good reference — production-grade kernel)

**What it is**: A production-grade Rust agentic runtime with a frame-based microservice architecture. The most sophisticated of the three — a full kernel with subsystems (Door, Room, LLM, VFS, EMS, Cache) communicating via async channels with request/response correlation.

| Attribute       | Value                                                                                  |
| --------------- | -------------------------------------------------------------------------------------- |
| Language        | Rust (tokio, edition 2024)                                                             |
| LOC             | ~118,600 across 657 files                                                              |
| Agent framework | Custom kernel with frame-based routing                                                 |
| Tools           | 1 generic "syscall" tool — LLM calls `syscall(name, data)` and kernel routes by prefix |
| LLM providers   | Anthropic + OpenAI, direct HTTP via reqwest                                            |
| Agent loop      | ReAct in `run_actor_loop()`, max 20 rounds                                             |
| Memory          | Per-room `Vec<HistoryEntry>` with optional SQLite persistence                          |
| Observability   | Structured tracing, typed error codes (`E_NOT_FOUND`, etc.)                            |
| Verification    | Syscall allowlist, round limits, deadlock prevention, error code enrichment            |

**Key patterns relevant to Ghostfolio agent**:

- **Single generic tool pattern**: Instead of registering 10 tools with the LLM, register one `syscall` tool and let the LLM specify the operation name + parameters. The kernel routes by name. This drastically simplifies tool registration and lets you add new tools without changing the LLM tool list. **Trade-off**: The LLM gets less schema guidance per tool, which may reduce accuracy.
- **History → messages conversion**: `history_to_messages()` with chrono gap markers (inject time context if >10 min between messages). Useful for long-running sessions where the LLM needs temporal awareness.
- **Typed error codes**: Every error carries a grepable `E_*` code + retryable flag. Agent can decide whether to retry based on error type, not just error text.
- **Pipe/Caller abstraction**: Request/response correlation via parent_id. In a TypeScript context, this maps to Promise-based tool dispatch — send request, await response, correlate by ID.
- **Config layering**: Code defaults → workspace config → user overrides. Useful for per-user agent customization (which tools enabled, which model, etc.).

**What's NOT transferable**: The kernel routing system, frame protocol, subsystem isolation, per-room workers. This is an OS-level abstraction far beyond AgentForge scope.

### 12c. gauntlet-week-1 (Meh reference — simpler sprint project)

**What it is**: A collaborative whiteboard app with an embedded AI agent that can create/move/resize objects on a canvas. Built in a 1-week sprint (same format as AgentForge). Custom Rust + Leptos (WASM frontend).

| Attribute       | Value                                                                                 |
| --------------- | ------------------------------------------------------------------------------------- |
| Language        | Rust (Axum backend, Leptos WASM frontend)                                             |
| LOC             | ~41,600 (25,800 source + 15,800 test)                                                 |
| Agent framework | Custom — simplified from Prior                                                        |
| Tools           | 18 tools (create/move/resize/update board objects)                                    |
| LLM providers   | Anthropic + OpenAI, switchable via env vars                                           |
| Agent loop      | ReAct, max 10 iterations per prompt                                                   |
| Memory          | In-memory per-session HashMap, max 3 turns / 3k chars, cleared on reconnect           |
| Observability   | Per-frame trace objects with timing + tokens; rate limiting (requests + token budget) |
| Token tracking  | Input/output tokens per call, accumulated across iterations                           |

**Key patterns relevant to Ghostfolio agent (most directly applicable of the three)**:

- **Tool definition pattern**: Declarative `Vec<Tool>` with JSON schema, returned by a builder function. No execution logic mixed in. Schema → dispatch → handler is clean and simple. **This is the closest pattern to what you'd build in TypeScript.**
- **Agent loop structure**: The `handle_prompt_with_parent()` function (2,566 lines) is a complete, readable ReAct loop:
  1. Snapshot state → build system prompt → load memory
  2. Loop (max 10): call LLM → parse tool calls → execute each → collect results → loop
  3. Return: mutations + text response + trace
- **Session memory with truncation**: Smart truncation rules (max turns, max chars per message, max total chars, evict oldest). Prevents context window overflow without losing all history. Directly portable.
- **Rate limiting**: Per-client request limits + global limits + token budget per hour. Essential for production and directly required by AgentForge.
- **Trace structure**: Every LLM call and tool execution produces a trace object with `{ trace_id, span_id, kind, elapsed_ms, duration_ms, input_tokens, output_tokens, stop_reason }`. This covers AgentForge's observability requirements almost exactly.
- **Tool result format**: Standard `{ tool_use_id, content, is_error }` blocks sent back to the LLM. Errors become tool results (not crashes), allowing the LLM to self-correct.
- **System prompt safety**: User input wrapped in `<user_input>` tags with explicit instruction not to follow embedded instructions. Basic but effective prompt injection defense.

**What's NOT transferable**: Rust/Axum/Leptos specifics, whiteboard domain logic, WebSocket-based mutation broadcasting. But the **architecture** translates almost 1:1 to TypeScript.

### Cross-Reference: What These Projects Tell Us About Building on Ghostfolio

All three projects share a common agent architecture that is **language-agnostic in principle**:

```
1. Define tools as JSON schemas (name, description, parameters)
2. Implement tool execution as async functions that return structured results
3. Run a ReAct loop: call LLM → parse tool calls → execute → inject results → repeat
4. Cap iterations (10-20) to prevent runaway cost
5. Track tokens, latency, and errors per call
6. Return errors as tool results, not exceptions
7. Maintain conversation history with truncation rules
```

**This pattern works in any language.** The TypeScript equivalent using Vercel AI SDK or LangChain.js would look structurally identical — the tool definitions become TypeScript objects, the execution functions become async NestJS service calls, and the loop becomes a simple while loop with the LLM client.

**The critical insight for Ghostfolio**: The agent framework is not the hard part. Defining tools as JSON schemas, running a ReAct loop, and tracking tokens are all well-understood, framework-agnostic patterns with ~200-500 lines of glue code. **The hard part is the integration surface area** — making Ghostfolio's services callable from tool execution functions, which is covered in Section 8.

---

## 13. Reference Eval Frameworks: ai-trials & faber-trials

Two sibling projects provide direct reference material for the AgentForge eval framework requirement (50+ test cases, scoring, observability). Both were built by the same author and share architectural DNA.

### 13a. ai-trials (Generic LLM eval harness — Python)

| Attribute         | Value                                                                              |
| ----------------- | ---------------------------------------------------------------------------------- |
| Language          | Python 3.11+ (async-first)                                                         |
| LOC               | ~1,800 (src/)                                                                      |
| Dependencies      | openai, anthropic, httpx, pyyaml, click, rich                                      |
| Task format       | YAML with id, type, prompt, expected, tags, judge_criteria                         |
| Grading           | 3 types: exact match, contains/regex, LLM-as-judge                                 |
| Providers         | 7 implementations (OpenAI, Anthropic, xAI, OpenRouter, Ollama, Z.AI, CLI wrappers) |
| Models configured | 50+ (GPT-4o, Claude, Grok, Gemini, Llama, etc.)                                    |
| Result storage    | Dual-write: JSONL (streaming/crash recovery) + SQLite (querying)                   |
| CLI commands      | run, all, models, providers, tasks, ping                                           |
| Maturity          | Early (2 commits, ~32 tasks)                                                       |

**Architecture highlights**:

- **Provider abstraction**: `async def call(CompletionRequest) -> CompletionResponse` — clean interface for swapping models. New providers require zero changes to core logic.
- **Pluggable graders**: Abstract `Grader` base class with `ExactGrader`, `ContainsGrader`, `JudgeGrader`. Task type determines grader.
- **LLM-as-judge**: Sends task + response to a judge model with configurable criteria (id, description, scale). Returns normalized 0-1 score from multi-criterion evaluation. Judge cost tracked separately.
- **SQLite schema**: Pre-built views — `v_model_stats`, `v_task_stats`, `v_run_summary`. Indexes on run, task, model, timestamp, grade. Per-result fields: tokens_in, tokens_out, latency_ms, cost, grade_passed, grade_score, judge_scores.
- **Crash recovery**: JSONL is append-only during execution. SQLite writes are atomic. Run can be resumed or re-analyzed from either store.
- **Cost tracking**: Per-model cost_per_1m_input/output configured in YAML. Computed per-request and aggregated per-run.
- **Configuration-driven**: Adding new tasks or models requires zero code changes — only YAML files.

**Gaps**: No structured logging framework, no architecture docs, no extension guide.

### 13b. faber-trials (Specialized eval harness — TypeScript/Bun)

| Attribute      | Value                                                             |
| -------------- | ----------------------------------------------------------------- |
| Language       | TypeScript on Bun runtime                                         |
| LOC            | ~3,074 (harness/) + 7,967 (YAML tasks)                            |
| Dependencies   | openai (^4.68.0), yaml (^2.8.2), Bun built-ins                    |
| Task format    | YAML with id, type, category, goal, input, expected_output        |
| Grading        | 3-level hierarchy: typecheck → runtime → correctness              |
| Providers      | OpenRouter only (50+ models via OpenAI-compatible API)            |
| Result storage | JSONL + SQLite + AI-generated analysis narratives                 |
| Runners        | 3: single model, pipeline (drafter+verifier), chain (multi-model) |
| Maturity       | Research-grade (500+ runs, 95+ tasks, 64MB results DB)            |

**Architecture highlights**:

- **Three runners** with distinct patterns:
  - **Single runner**: Model × N-shots × Contexts × Tasks matrix. Standard eval.
  - **Pipeline runner**: Drafter (small model) → Verifier (large model). Measures "transitions": preserved/damaged/recovered/failed. Tests whether verification improves small-model output.
  - **Chain runner**: Judge → R1 → P2 → R2 → Verdict. Multi-model conversation chains with structured scoring criteria. Configurable depth (1-3 models).
- **Compile-based grading**: Level A (typecheck), Level B (runtime execution), Level C (output match). External verification via actual compiler — not string matching. Error taxonomy: syntax_error, type_error, runtime_error, wrong_output, no_response, api_error.
- **Reproducibility**: temperature=0.0, git SHA in every result, framework_version bumped when evaluation criteria change. Full prompts stored in raw_responses.jsonl.
- **Prompt construction**: Context levels (examples-only, minimal, basic, complete) control how much reference material the model receives. N-shot configurable (0, 1, 3, 10).
- **AI-generated analysis**: analyzer.ts calls Claude Haiku to produce narrative summaries of run results.
- **Transition tracking**: For pipeline runs, tracks whether verifier preserved, damaged, recovered, or failed the drafter's output. Maps well to evaluating agent tool call chains.

**Gaps**: Tightly coupled to Faber language domain; not directly reusable as-is.

### Comparison for AgentForge Applicability

| Aspect                   | ai-trials                           | faber-trials                               | AgentForge Need                                                                 |
| ------------------------ | ----------------------------------- | ------------------------------------------ | ------------------------------------------------------------------------------- |
| **Language**             | Python                              | TypeScript/Bun                             | TS preferred (Ghostfolio stack)                                                 |
| **Task format**          | YAML (generic)                      | YAML (domain-specific)                     | YAML is ideal                                                                   |
| **Grading approach**     | String match + LLM-as-judge         | Compile-based (external verifier)          | Both useful: string match for tool selection, LLM-as-judge for response quality |
| **Multi-model support**  | 7 providers, 50+ models             | OpenRouter only, 50+ models                | Any provider works; Anthropic primary                                           |
| **Pipeline/chain evals** | No                                  | Yes (drafter+verifier, multi-model chains) | Useful for verification layer testing                                           |
| **Result storage**       | JSONL + SQLite (dual-write)         | JSONL + SQLite + AI narrative              | JSONL + SQLite pattern is proven                                                |
| **Cost tracking**        | Per-request + aggregated            | Per-request + aggregated                   | Required by AgentForge                                                          |
| **Observability**        | Latency, tokens, cost, verbose mode | Latency, tokens, cost, error taxonomy      | Both meet requirements                                                          |
| **Portability**          | Python → must port patterns         | TypeScript → patterns port directly        | faber-trials is closer to target                                                |

### What to Extract for Ghostfolio Agent Eval

**From ai-trials** (pattern reference):

1. **Dual-write JSONL + SQLite** — proven crash-recovery + query pattern
2. **LLM-as-judge grading** — essential for evaluating agent response quality (not just tool correctness)
3. **SQLite schema with pre-built views** — model stats, task stats, run summaries out of the box
4. **Configuration-driven task definitions** — YAML files, no code changes per task
5. **Provider abstraction** — clean interface if you want to test agent with different LLMs

**From faber-trials** (direct code reference):

1. **TypeScript harness structure** — same language as Ghostfolio, patterns copy directly
2. **Pipeline runner** — drafter+verifier maps to agent+verification layer testing
3. **Transition tracking** (preserved/damaged/recovered/failed) — apply to agent tool chain evaluation
4. **Prompt construction with context levels** — test how much system prompt context the agent needs
5. **Reproducibility practices** — git SHA in results, framework versioning, temperature=0.0

**Combined approach for AgentForge eval**:

- Define 50+ test cases in YAML (ai-trials format: id, type, prompt, expected, tags, judge_criteria)
- Build a TypeScript runner (port ai-trials pattern, reference faber-trials code)
- Grade with: exact match for tool selection correctness, contains for required data in responses, LLM-as-judge for response quality
- Store results: JSONL streaming + SQLite with aggregate views
- Track per-test: latency_ms, tokens_in, tokens_out, cost, grade_passed, judge_score
- Pipeline variant: test agent response → verification layer → measure transition quality

**Estimated effort to build eval framework with these references: 6-12 hours** (reduced from "from scratch" estimate because the patterns, schemas, and TypeScript reference code already exist).

---

## 14. Local Development & Deployment

### Local Development Setup (15-30 minutes, low risk)

**Prerequisites**: Node.js >= 22.18.0 (confirmed: local machine runs v24.6.0), Docker.

**Steps**:

1. Copy `.env.dev` to `.env`, replace 4 placeholders with any random strings:

   ```
   REDIS_PASSWORD=anything
   POSTGRES_PASSWORD=anything
   ACCESS_TOKEN_SALT=any-random-string
   JWT_SECRET_KEY=any-random-string
   ```

2. Start Postgres + Redis via Docker:

   ```
   docker compose -f docker/docker-compose.dev.yml up -d
   ```

3. Install dependencies and set up database:

   ```
   npm install
   npm run database:setup   # prisma db push + seed
   ```

4. Start dev servers (two terminals):
   ```
   npm run start:server     # NestJS API on port 3333
   npm run start:client     # Angular dev server with HMR
   ```

**No API keys required for basic operation.** Yahoo Finance works without a key (scraping-based), so you get live market data out of the box. No Stripe key needed (premium features just won't activate). No OAuth keys needed (anonymous/local auth works).

**No exotic dependencies.** Just Postgres and Redis in Docker, which the dev compose file handles. The `database:setup` command pushes the Prisma schema and seeds initial data automatically.

### Production / Demo Deployment (30-60 minutes, low risk)

**Recommended approach: Docker build from source.**

```
docker compose -f docker/docker-compose.build.yml up -d
```

This builds the Dockerfile from your fork, then starts the full stack (app + Postgres + Redis). The entrypoint script automatically runs `prisma migrate deploy` + `prisma db seed` on every container start. Health checks are built in for all three services.

**What makes deployment simple**:

- **Single port (3333)** — NestJS serves both the API and the Angular static build from one process
- **Standard infrastructure** — just Postgres + Redis, nothing exotic
- **Entrypoint handles migrations** — no manual database setup on deploy
- **Health check endpoint** — `GET /api/v1/health` for load balancer integration
- **No external services required** — Yahoo Finance works without keys for basic market data

**Works on any Docker-capable host**: DigitalOcean droplet, AWS EC2, Hetzner, Railway (auto-detects Dockerfile), Fly.io, Render, any VPS.

### Build Time Comparison

The Ghostfolio Dockerfile is a multi-stage Node build: `npm install` in the builder stage, then `npm run build:production` (Nx compiles API + client + Storybook). This is significantly faster than a Rust `cargo build --release`.

| Project         | Build Type                           | Expected Build Time |
| --------------- | ------------------------------------ | ------------------- |
| gauntlet-week-1 | Rust full compilation + WASM target  | 10-20+ minutes      |
| Ghostfolio      | Node npm install + Nx/webpack bundle | 3-5 minutes         |

The Dockerfile does install `g++`, `make`, and `python3` for native Node modules (node-gyp), but these are just for compiling a few C bindings, not a full language toolchain build.

### Deployment Gotchas

- **Memory during build**: Nx + Angular + NestJS compilation is memory-hungry. A 1GB RAM VPS will likely OOM during the Docker build. Need 2GB+ RAM, or build locally and push the image to a registry.
- **No pre-built image for your fork**: The official `ghostfolio/ghostfolio:latest` is for upstream. You must build your own image via `docker-compose.build.yml` or push to your own registry.
- **SSL/HTTPS not included**: Ghostfolio serves plain HTTP on port 3333. For a public demo, you need a reverse proxy (Caddy is simplest — auto-HTTPS with Let's Encrypt) or a PaaS that terminates SSL.
- **Postgres volume persistence**: `docker compose down -v` destroys the database. For a demo this is fine; for anything longer, back up the volume or use a managed database.
- **Build cache**: Subsequent Docker builds are fast if only your agent code changed (npm install layer is cached). But changing `package.json` invalidates the cache and triggers a full reinstall.

### Bottom Line

**This is one of Ghostfolio's genuine strengths.** The DevOps story is clean and well-maintained. You can go from fork to running local dev in 15 minutes, and from local dev to public demo deployment in under an hour. No exotic infrastructure, no paid service dependencies, no complex configuration. The Docker build is lightweight compared to compiled-language projects.

---

## 15. Developer Profile: Self-Assessment

This section captures relevant prior experience that materially changes the risk/effort estimates throughout this document.

### TypeScript Expertise

**monk-api** (`../monk-api`) — a 59,000-line strict-mode TypeScript backend built on Hono:

- Direct Anthropic API integration with tool use (Claude Sonnet 4)
- MCP (Model Context Protocol) server implementation
- Custom SQL query builder (no ORM) with PostgreSQL + SQLite
- Multi-tenant architecture with schema-per-tenant isolation
- Ring-based observer/hook system (similar to NestJS interceptors/guards)
- Field-level change tracking, record-level ACLs
- `strict: true`, advanced generics, async generators, path aliases

This eliminates the "TypeScript learning curve" risk entirely and reduces the "NestJS learning curve" to orientation on NestJS-specific conventions (module registration, decorator syntax, request scoping). The underlying patterns — DI, middleware, guards, async services — are already familiar.

### Agentic Implementation Experience

Three prior agentic systems built in Rust (see Section 12):

- **abbot**: 39k LOC, 33 tools, multi-agent with parallel execution
- **prior**: 118k LOC, production kernel with frame-based tool dispatch
- **gauntlet-week-1**: 41k LOC, 18 tools, ReAct loop, same sprint format as AgentForge

Plus **monk-api**: Direct Anthropic tool calling in TypeScript — the exact integration pattern needed for Ghostfolio.

This eliminates the "agent framework integration" risk. The ReAct loop, tool schema definitions, token tracking, session memory with truncation, rate limiting, and trace objects have all been implemented before — multiple times, in multiple languages, including TypeScript.

### What Remains Genuinely New

- NestJS-specific conventions (half-day orientation)
- Ghostfolio's service signatures and data model (the integration surface area from Section 8)
- Angular (only if a frontend chat UI is needed; avoidable for MVP)
- Prisma (straightforward ORM, not a major learning curve)
- The eval framework (50+ test cases — tedious but no technical unknowns)

### Revised Difficulty Ratings

| Aspect                    | Generic Rating     | With This Profile        | Why                                                                 |
| ------------------------- | ------------------ | ------------------------ | ------------------------------------------------------------------- |
| NestJS learning curve     | Hard               | **Low-Medium**           | Same DI/decorator/guard patterns as monk-api, just different syntax |
| Agent loop + tool calling | Medium             | **Low**                  | Built this exact thing in monk-api (TS + Anthropic) and 3x in Rust  |
| Integration surface area  | Medium             | **Medium** (unchanged)   | Still need to learn Ghostfolio's specific service APIs              |
| Observability             | Medium             | **Low**                  | gauntlet-week-1 trace pattern ports directly                        |
| Frontend chat UI          | Hard               | **Medium-Hard**          | Angular is still Angular, but a minimal endpoint may suffice        |
| Overall 7-day feasibility | Tight but feasible | **Feasible with margin** | 43-86 hour estimate vs ~112 waking hours                            |

---

## 16. Summary

**Ghostfolio is a mature, well-maintained, TypeScript-only personal finance platform with rich domain logic that maps well to the AgentForge finance tool requirements.** The portfolio calculator, rules engine, and market data providers are genuine assets that would give your agent meaningful, real capabilities.

**The project brings significant complexity** — a full NestJS + Angular + Nx + Prisma monorepo with 55,000 lines of TypeScript, 107 database migrations, enterprise authentication, and a Stripe subscription system. There is no plugin architecture, no existing agent infrastructure worth building on, and no test safety net.

**However, given the developer profile (Section 15), the two biggest risks are eliminated:**

1. ~~TypeScript/NestJS learning curve~~ → Experienced TS developer with a 59k-line strict-mode backend (monk-api). NestJS orientation is half a day, not half the sprint.
2. ~~Agent framework integration~~ → Already built direct Anthropic tool calling in TypeScript (monk-api) and three full agentic systems in Rust. The ReAct loop, tool schemas, token tracking, and session memory are all solved problems.

**What remains is the integration surface area** — understanding Ghostfolio's service signatures well enough to wrap them as tools. Section 8 shows this is manageable: 5 services are directly callable singletons, PortfolioService is reachable via HTTP self-calls, and 7-8 working tools are realistic without modifying any existing Ghostfolio code.

**Estimated effort: 43-86 hours for all AgentForge deliverables, against ~112 available waking hours.** This leaves comfortable margin for iteration, debugging, and the eval framework's 50+ test cases.

**Verdict: Ghostfolio is a strong choice for this sprint.** The combination of TypeScript expertise, prior agentic implementation experience, rich wrappable domain logic, and clean deployment story makes it feasible with margin — not just "tight but possible."
