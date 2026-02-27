# TUI Design: Bloomberg-Style Terminal Client

**Status:** Draft v2 — revised layout & color refinements
**Location:** `gauntlet/cli/`
**Stack:** Ink 5 + React 18 + TypeScript (ESM)

---

## Concept

A Bloomberg Terminal-inspired TUI for interacting with the Ghostfolio Agent over HTTP. Dark background, monospace, information-dense panels. Amber/white/green on black — with color used semantically, not decoratively. The terminal _is_ the product for the MVP demo — no Angular UI changes needed.

Connects to the deployed API at `POST /api/v1/agent/chat` using JWT authentication.

### Bloomberg Design Principles

The aesthetic borrows these specific traits from the Bloomberg Terminal:

- **Amber is structural.** Headers, labels, borders, prompts — anything that is chrome, not data — renders in amber (#FF8800). This is Bloomberg's signature color dating to 1980s amber phosphor CRTs.
- **White is data.** All primary content (user input, agent responses, values) renders in white or near-white. The eye learns to skip amber (structure) and read white (content).
- **Color is semantic.** Green = positive/success. Red = negative/failure. Amber = chrome/labels. No decorative color. Every hue carries meaning.
- **Density is the feature.** No decorative whitespace. Every row has data. Tight column alignment in fixed-width fields.
- **Monospace everything.** Column alignment is pixel-perfect via fixed-width character grids.

---

## Layout (ASCII Wireframe)

### Primary View — Sidebar Layout

The main content area uses a two-column layout: chat on the left (flex-grow), metadata sidebar on the right (fixed ~28 chars). This gives chat full vertical height while keeping tools/session data persistently visible.

```
┌─ GHOSTFOLIO AGENT ──────────────────────────────────────────────────────────┐
│                                                                              │
│  ┌─ CHAT ──────────────────────────────────────────┐  ┌─ TOOLS ──────────┐  │
│  │  YOU  What accounts do I have?                   │  │  market_data      │  │
│  │                                                  │  │    312ms ✓        │  │
│  │  AGT  You have 2 accounts:                       │  │  market_bench..   │  │
│  │       1. Interactive Brokers (USD) — 12 holdings  │  │    208ms ✓        │  │
│  │       2. Savings Account (EUR) — 0 holdings       │  │  acct_overview    │  │
│  │                                                  │  │    156ms ✓        │  │
│  │  YOU  What is the current price of AAPL?         │  │  exchange_rate    │  │
│  │                                                  │  │     45ms ✓        │  │
│  │  AGT  AAPL (Apple Inc.) is currently trading at  │  ├─ SESSION ────────┤  │
│  │       $189.84, up +1.23% today. Market is open.  │  │  Model  sonnet-.. │  │
│  │                                                  │  │  Turn   4         │  │
│  │  YOU  Show me portfolio vs S&P 500               │  │  Tkn In 2,481     │  │
│  │                                                  │  │  Tkn Out 612      │  │
│  │  AGT  Your portfolio performance comparison:     │  │  Latency 2,104ms  │  │
│  │       Your Portfolio:  +14.2% YTD                │  │  Steps  3         │  │
│  │       S&P 500:         +12.8% YTD                │  │  Verified ✓       │  │
│  │       Alpha:           +1.4%                     │  ├─ AVAILABLE ──────┤  │
│  │                                                  │  │  account_overview │  │
│  │       Your portfolio is outperforming the        │  │  market_data      │  │
│  │       benchmark by 140bps YTD, driven by NVDA.   │  │  exchange_rate    │  │
│  │                                                  │  │  symbol_lookup    │  │
│  │                                                  │  │  asset_profile    │  │
│  │                                                  │  │  market_benchmrks │  │
│  │                                                  │  │  price_history    │  │
│  ├──────────────────────────────────────────────────┤  └──────────────────┘  │
│  │  >>> _                                           │                        │
│  └──────────────────────────────────────────────────┘                        │
│                                                                              │
│  F1 Help  F2 New Session  F5 Reconnect  F10 Quit                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Waiting State — LLM Processing

```
│  │  ⏳ Thinking...                                  │  ┌─ TOOLS ──────────┐  │
│  │                                                  │  │  ⏳ market_data   │  │
│  │                                                  │  │    running..      │  │
│  │                                                  │  ├─ SESSION ────────┤  │
│  │                                                  │  │  Status ● Active  │  │
```

### Error State

```
│  │  AGT  ⚠ Tool error: exchange_rate failed —       │  ┌─ TOOLS ──────────┐  │
│  │       rates not loaded.                          │  │  exchange_rate    │  │
│  │       The agent could not complete your request.  │  │    128ms ✗        │  │
```

### Verification Warning

```
│  │  AGT  Bitcoin is currently trading at             │  ┌─ TOOLS ──────────┐  │
│  │       approximately $67,000.                     │  │  (none)           │  │
│  │                                                  │  ├─ SESSION ────────┤  │
│  │  ⚠ UNVERIFIED — Agent answered a data question   │  │  Verified ⚠       │  │
│  │    without consulting any tools. Response may    │  │                    │  │
│  │    contain unsupported claims.                   │  │                    │  │
```

---

## Panels

### 1. Chat Panel (left column, flex-grow)

- Scrollable message history, full vertical height
- Message prefixes in amber: `YOU`, `AGT` — labels are structural chrome
- User input text in white, agent response text in white
- Markdown in agent responses rendered as plain text (strip formatting or minimal: bold, lists)
- Spinner (`⏳ Thinking...`) in amber while awaiting response
- Verification warnings rendered inline in amber/yellow with `⚠` prefix

### 2. Tools Panel (right sidebar, top section)

- Shows tool calls from the **most recent turn only**
- Each entry: tool name on one line, latency + status below (sidebar is narrow)
- Status: `✓` green (ok: true), `✗` red (ok: false), spinner while running
- Long tool names truncated to fit sidebar width (e.g., `market_bench..`)
- Clears on each new user message

### 3. Session Panel (right sidebar, middle section)

- Metadata from the most recent `AgentChatResponse`:
  - `Model` — model name (truncated for sidebar)
  - `Turn` — `turnNumber`
  - `Tkn In` — `tokenUsage.input`
  - `Tkn Out` — `tokenUsage.output`
  - `Latency` — `durationMs`
  - `Steps` — `steps.length`
  - `Verified` — `✓` or `⚠` based on `verified` flag
- Labels left-aligned in amber, values right-aligned in white

### 4. Available Tools Panel (right sidebar, bottom section)

- Static list of all 7 agent tools the server exposes
- Renders once on startup, does not change per-turn
- Serves as a cheat sheet: the user can see what the agent is capable of
- Tool names in muted gray — this is reference info, not active data

### 5. Input Bar (below chat, left column only)

- Single-line text input with `>>> ` prompt in amber
- Enter submits, input disabled while processing
- Up arrow for history recall (optional, nice-to-have)

### 6. Status Bar (very bottom, full width)

- Fixed single line showing keyboard shortcuts
- Keys in amber, descriptions in muted gray
- `F1 Help  F2 New Session  F5 Reconnect  F10 Quit`

---

## Color Theme (Bloomberg-Semantic)

Colors are assigned by semantic role, not by UI element. Every color has a single meaning.

```
Background:        #000000 (pure black)
Structural chrome: #FF8800 (amber — headers, labels, borders, prompts, prefixes)
Data / content:    #FFFFFF (white — user text, agent text, values)
Positive / ok:     #00FF00 (green — tool ✓, verified ✓, gains)
Negative / error:  #FF4444 (red — tool ✗, failures, losses)
Warning:           #FFAA00 (amber/yellow — verification warnings)
Muted / reference: #888888 (gray — metadata values, available tools, session ID)
Panel borders:     #444444 (dark gray)
```

Removed from v1: cyan spinner (replaced with amber), green agent text (replaced with white). The palette is now 6 semantic colors + 2 grays.

---

## API Integration

### Authentication Flow

1. On startup, prompt for server URL (default: `http://localhost:3333`)
2. Login via `POST /api/v1/auth/anonymous` or accept a pre-configured JWT via env var (`GHOSTFOLIO_TOKEN`)
3. Store JWT in memory for session lifetime

### Chat Loop (NDJSON Streaming)

The endpoint streams NDJSON (one JSON object per line). Step events arrive as the agent processes each tool call, followed by a final `done` event with the complete response.

```
User types message
  → POST /api/v1/agent/chat { sessionId?, message, model? }
  → Headers: { Authorization: Bearer <jwt> }
  ← Content-Type: application/x-ndjson
  ← {"type":"step","step":{"stepNumber":1,"toolCalls":[...],"tokenUsage":{...},"durationMs":512}}
  ← {"type":"step","step":{"stepNumber":2,"toolCalls":[...],"tokenUsage":{...},"durationMs":234}}
  ← {"type":"done","response":{ ...full AgentChatResponse... }}
  → On each "step" line: update Tools panel live
  → On "done" line: update Chat, Session, and all panels with final data
  → Store sessionId for next request
```

Error events may also be emitted:

```
  ← {"type":"error","error":"error message"}
```

### Stream Event Types (from agent.interfaces.ts)

```typescript
type AgentStreamEvent =
  | { type: 'step'; step: StepRecord }
  | { type: 'done'; response: AgentChatResponse }
  | { type: 'error'; error: string };

interface AgentChatResponse {
  sessionId: string;
  model: string;
  response: string;
  toolCalls: ToolCallRecord[];
  steps: StepRecord[];
  turnNumber: number;
  verified: boolean;
  verificationWarning?: string;
  durationMs: number;
  tokenUsage: { input: number; output: number };
}

interface StepRecord {
  stepNumber: number;
  toolCalls: ToolCallRecord[];
  tokenUsage: { input: number; output: number };
  durationMs: number;
}

interface ToolCallRecord {
  tool: string;
  parameters: Record<string, unknown>;
  result: { ok: boolean; data?: unknown; error?: string };
  durationMs: number;
}
```

---

## Available Agent Tools (7)

These are the tools the agent can invoke server-side. The TUI doesn't call them directly — it displays what the agent used in the Tools panel, and lists all available tools in the Available panel.

| Tool                | Description                                                 |
| ------------------- | ----------------------------------------------------------- |
| `account_overview`  | List all user accounts with balances, currencies, platforms |
| `market_data`       | Current quotes (price, currency, market state) for symbols  |
| `exchange_rate`     | Convert amount between currencies                           |
| `symbol_lookup`     | Search for symbols by name/keyword                          |
| `asset_profile`     | Detailed profile for a specific asset                       |
| `market_benchmarks` | Benchmark/index performance data                            |
| `price_history`     | Historical price data for a symbol over a date range        |

---

## Component Tree (Ink/React)

```
<App>
  <Box flexDirection="column" height="100%">
    <Header title="GHOSTFOLIO AGENT" />

    <Box flexDirection="row" flexGrow={1}>
      {/* Left: Chat + Input */}
      <Box flexDirection="column" flexGrow={1}>
        <ChatPanel messages={messages} loading={loading} />
        <InputBar onSubmit={handleSubmit} disabled={loading} />
      </Box>

      {/* Right: Sidebar */}
      <Box flexDirection="column" width={28}>
        <ToolsPanel toolCalls={lastResponse?.toolCalls} />
        <SessionPanel response={lastResponse} />
        <AvailableToolsPanel tools={AGENT_TOOLS} />
      </Box>
    </Box>

    <StatusBar />
  </Box>
</App>
```

---

## Keyboard Shortcuts

| Key       | Action                                  |
| --------- | --------------------------------------- |
| `Enter`   | Submit message                          |
| `F1`      | Show help overlay                       |
| `F2`      | Clear session, start new conversation   |
| `F5`      | Reconnect (re-authenticate)             |
| `F10`     | Quit                                    |
| `Ctrl+C`  | Quit                                    |
| `Up/Down` | Scroll chat history (when not in input) |
| `Esc`     | Close overlay / cancel                  |

---

## Project Structure

```
gauntlet/cli/
├── package.json              # standalone package, ESM
├── tsconfig.json             # ESM output, JSX react-jsx
├── src/
│   ├── index.tsx             # entry point, Ink render(<App />)
│   ├── app.tsx               # root layout, state management
│   ├── api.ts                # HTTP client (fetch + JWT)
│   ├── theme.ts              # bloomberg color constants (semantic roles)
│   ├── components/
│   │   ├── header.tsx        # top bar with title
│   │   ├── chat-panel.tsx    # scrollable message list
│   │   ├── tools-panel.tsx   # tool call display (sidebar)
│   │   ├── session-panel.tsx # metadata display (sidebar)
│   │   ├── available-panel.tsx # static tool list (sidebar)
│   │   ├── input-bar.tsx     # text input with prompt
│   │   └── status-bar.tsx    # bottom shortcut hints
│   └── types.ts              # response types (mirrored from agent.interfaces.ts)
└── bin/
    └── ghostfolio-cli.js     # shebang entry: node --loader tsx src/index.tsx
```

---

## Dependencies

```json
{
  "dependencies": {
    "ink": "^5.0.0",
    "ink-text-input": "^6.0.0",
    "ink-spinner": "^5.0.0",
    "react": "^18.3.0"
  },
  "devDependencies": {
    "typescript": "^5.5.0",
    "tsx": "^4.0.0",
    "@types/react": "^18.3.0"
  }
}
```

---

## Startup Flow

```
$ cd gauntlet/cli && npx tsx src/index.tsx

   -- or --

$ GHOSTFOLIO_URL=https://ghostfolio.example.com GHOSTFOLIO_TOKEN=ey... npx tsx src/index.tsx
```

1. Read `GHOSTFOLIO_URL` (default `http://localhost:3333`) and `GHOSTFOLIO_TOKEN` from env
2. If no token, prompt for anonymous auth or show error
3. Render full TUI
4. Focus input bar, ready for first message

---

## Open Design Questions

- **Streaming:** The agent endpoint streams NDJSON step events as tool calls complete. The TUI should read these line-by-line and update the Tools panel live. The final `done` event carries the full response. Token-by-token text streaming is not yet supported — the chat panel shows a spinner until the `done` event, then renders the complete response text.
- **Chat export:** Should there be a way to dump the conversation to a file? (e.g., `F3 Export`)
- **Multi-model selector:** The API accepts an optional `model` param. Could add a model picker overlay.
- **Resize handling:** Ink handles terminal resize events natively via Yoga layout. The sidebar should maintain its fixed width; chat column absorbs all resize changes. Minimum usable terminal width is ~80 columns (52 chat + 28 sidebar).
- **Step-by-step display:** The `steps[]` array has per-step timing and tool calls. Could expand the Tools panel into a scrollable step-by-step breakdown on demand (e.g., press `Tab` to toggle detail view in the sidebar).
- **Sidebar truncation:** Tool names longer than the sidebar width need truncation. Decide on a convention: trailing `..` (e.g., `market_bench..`) or abbreviation (e.g., `mkt_benchmarks`). Trailing `..` is simpler and more consistent.
