# Intelligent Terminal — Induction Analysis

> Mapping the ensign system capabilities for SuperInstance integration.

## Architecture Overview

Intelligent Terminal is a Microsoft Windows Terminal fork with a deeply layered analysis and agent orchestration system built in Rust. The `wta` tool (the analysis engine) sits between the terminal event loop and the user, providing real-time mathematical analysis, command forecasting, and multi-agent coordination.

## Module Map

### 1. Math Analysis (`math_analysis/`)

#### `command_markov.rs` — Ergodic Command Analysis
Builds a first-order Markov chain from command-to-command transitions. Computes the **stationary distribution** (long-run fraction of time in each command state), detects temporal anomalies via z-score deviation from stationary expectations, and estimates **mixing time** (steps to reach ergodic convergence). Uses `nalgebra::DMatrix` for dense matrix operations. Up to 512 tracked command states.

#### `error_hodge.rs` — Hodge Decomposition of Errors
Decomposes errors into three orthogonal components:
- **Evidence** — raw signal strength (what happened)
- **Coherence** — internal consistency of the error message
- **Prior mismatch** — divergence between user expectation and reality

Produces a dominant classification (`Evidence`, `Incoherence`, `PriorMismatch`) that downstream autofix pipelines can route on. This is a *deterministic* decomposition — no ML, no model calls.

#### `verification_entropy.rs` — Conservation of Verification Entropy
Thermodynamic metaphor: every edit without running tests increases entropy. Formula: `E = 1 - exp(-α · edits_since_last_test / L)`. Four severity levels (Low → Critical). Tracks cumulative lines edited vs tests run. The "conservation" is that entropy is neither created nor destroyed — it just shifts from "untested code" to "known bugs."

#### `spectral_dashboard.rs` — Agent Collaboration Network Analysis
Builds a graph where nodes = agents (Copilot, Claude, Codex, Gemini) and edges = shared context (same project, overlapping file edits). Computes:
- **Fiedler value** (λ₂) — algebraic connectivity
- **Cheeger constant** — isoperimetric bottleneck detection
- **Mixing time** — information spread latency

Renders a compact status bar: `λ₂=0.34 h=0.21`

### 2. Module System (`module_system/`)

#### `mod.rs` — Plugin Architecture
Trait-based plugin system. `TerminalModule` trait defines the lifecycle: `trigger()` → `activate()` → `handle_event()` → `deactivate()`. Modules are **guests** — they can observe and suggest but never commandeer the UI or block the event loop. Events are typed (`CommandEntered`, `CommandCompleted`, `Error`, `DirectoryChanged`, `AgentStarted`, `AgentEnded`, `Tick`).

#### `builtin_modules.rs` — Trait Implementations
Wraps each analysis type behind `TerminalModule`:
- `CommandMarkovModule` — triggers on `CommandEntered`
- `ErrorHodgeModule` — triggers on `Error` and non-zero exits
- `VerificationEntropyModule` — tracks edit/test ratio
- `SpectralDashboardModule` — activates when 2+ agent panes open

Feature-gated: `math-tools` and `griot-history`.

#### `module_context.rs` — Sandboxed Context
Read-only snapshot of terminal state exposed to modules: command history (last 200), working directory, active agent IDs, last error, project files. Modules cannot access filesystem, network, or agent internals.

#### `module_output.rs` — Advisory Outputs
Five output types ranked by weight: `StatusBar` (1), `InlineHint` (2), `Notification` (3), `BarChart` (4), `Insight` (5). All advisory — the terminal decides what to show.

#### `memory_budget.rs` — Resource Management
50MB total budget across all modules. LRU eviction when exceeded — least-recently-triggered modules are deactivated and serialized to disk. Reactivation deserializes state back.

### 3. Context Trigger Engine (`context_trigger/`)

#### `mod.rs` — Lazy Activation Engine
Zero-overhead module loading. Each module registers a pure `fn(&TerminalEvent) -> bool` trigger. Dormant modules consume zero memory. Target: <1ms latency added to event loop. States: `Dormant` → `Triggered` → `Active` → `Expired` → `Dormant`.

#### `triggers.rs` — Pure Trigger Predicates
Stateless, allocation-free trigger functions:
- `math_tools_trigger` — fires on test commands or 2+ agent panes
- `griot_history_trigger` — fires when history >50 commands
- `griot_history_pattern_trigger` — fires when history >200 commands
- `verification_entropy_trigger` — fires on successful builds
- `error_hodge_trigger` — fires on non-zero exits
- `spectral_dashboard_trigger` — fires on 2+ panes or explicit disagreement

#### `autoconfig.rs` — Zero-Config Detection
Scans PATH at startup for cargo, python, node, and agent CLIs (copilot, claude, codex, gemini). Auto-enables triggers based on detected tooling. No config files, no UI, no user input.

#### `dormant.rs` — Lifecycle State Machine
Four-state FSM with strict transition rules. Single-threaded by design. `ModuleState` is `Send` but not `Sync` — shared via `Rc<RefCell<>>` within the event loop thread.

### 4. Forecast System (`forecast/`)

#### `transition_matrix.rs` — Markov Chain Engine
100-state first-order Markov chain with Laplace smoothing (α=1). Row-normalized stochastic matrix. Serialize/deserialize for cross-session persistence. Collapses rare commands into `__other__`.

#### `predictor.rs` — Next-Command Prediction
Top-K predictions with confidence scores from `P(next | current)`. Generates ghost text for autocomplete: `cargo test  cargo run  cargo clippy`.

#### `anomaly.rs` — Workflow Shift Detection
KL divergence + Wasserstein-1 distance between current and stationary distributions. Four severity levels: Normal → Dramatic. Tracks historical divergence for trend analysis.

#### `resource_predictor.rs` — Resource Usage Forecasting
Exponential moving averages of memory, CPU, and duration per command. Predicts resource needs *before* execution and warns when predicted usage exceeds available capacity.

### 5. UI System (`ui/`)

Full ratatui-based TUI with:
- `layout.rs` — Main layout engine with adaptive sizing
- `chat.rs` — Agent chat view with streaming support
- `agents_view.rs` — Session registry view with Figma-style color palette
- `agent_disagreement.rs` — **Sheaf-theoretic** disagreement visualization (H⁰ connectedness, H¹ structural obstructions)
- `entropy_bar.rs` — Always-visible verification entropy indicator
- `input.rs` — Multi-line input with wrapping and cursor tracking
- `command_popup.rs` — Slash-command autocomplete
- `recommendations.rs` — Action recommendation cards with scroll
- `permission.rs` — Permission prompt cards
- `debug_panel.rs` — Protocol debug view (F12)
- `shimmer.rs` — Animated text sweep effect
- `card.rs` — Reusable card component system
- `auth.rs` — Agent authentication flow
- `setup.rs` — First-run experience wizard

### 6. Agent System

#### `agent_registry.rs` — Static Agent Catalog
Complete profiles for each agent CLI: executable resolution, ACP server flags, delegate prompt delivery, display names, model selection, authentication flow. Adding a new agent = adding one entry to `KNOWN_AGENTS`.

#### `agent_sessions.rs` — Runtime Session Registry
Tracks live and historical agent sessions. Two GUID systems: `pane_session_id` (Windows Terminal pane) and `key` (agent's own session ID for resume). Supports Claude, Codex, Copilot, Gemini. Synthesizes keys for agents without hook support.

#### `coordinator.rs` — Orchestration Layer
Manages agent lifecycle: delegate launch, ACP connections, recommendation sets, model selection. Handles `RecommendationChoice` with `RecommendedAction` routing. The central orchestrator connecting the UI, agent sessions, and analysis modules.

## Classification Map

### Ensign-Like (Agent Lifecycle, Capability Registration)
- **`agent_registry.rs`** — Static capability catalog (like ensign's service registry)
- **`agent_sessions.rs`** — Runtime session tracking (like ensign's instance management)
- **`module_system/mod.rs`** — Trait-based plugin registration (like ensign's handler registration)
- **`context_trigger/dormant.rs`** — Lifecycle state machine (like ensign's pod lifecycle)
- **`context_trigger/autoconfig.rs`** — Auto-discovery (like ensign's service discovery)

### Lever-Runner-Like (Command Execution)
- **`coordinator.rs`** — Delegates commands to agents, manages execution
- **`forecast/predictor.rs`** — Predicts and suggests next commands
- **`module_system/builtin_modules.rs`** — Wraps analysis into executable handlers
- **`ui/input.rs`** — Command input capture and routing

### PincherOS-Like (State, Memory, Reflexes)
- **`forecast/transition_matrix.rs`** — Persistent Markov chain state
- **`math_analysis/command_markov.rs`** — Cumulative transition history
- **`module_system/memory_budget.rs`** — Resource-aware state management with LRU eviction
- **`agent_sessions.rs`** — Session history and resume capability
- **`forecast/resource_predictor.rs`** — Historical resource tracking

### PLATO-Like (Orchestration)
- **`coordinator.rs`** — Central orchestration of agents, recommendations, actions
- **`context_trigger/mod.rs`** — Event routing and module activation
- **`math_analysis/spectral_dashboard.rs`** — Multi-agent collaboration analysis
- **`ui/agent_disagreement.rs`** — Conflict resolution via sheaf cohomology
- **`ui/layout.rs`** — Adaptive layout orchestration

## Connection Graph

```
Terminal Event Loop
    │
    ├── context_trigger (lazy activation)
    │       │
    │       └── module_system (plugin lifecycle)
    │               │
    │               ├── math_analysis (analysis engines)
    │               │       ├── command_markov ← feeds forecast
    │               │       ├── error_hodge ← feeds error suggestions
    │               │       ├── verification_entropy ← feeds entropy_bar UI
    │               │       └── spectral_dashboard ← feeds agent_disagreement UI
    │               │
    │               ├── forecast (prediction engines)
    │               │       ├── transition_matrix ← seeded by command_markov
    │               │       ├── predictor ← reads transition_matrix
    │               │       ├── anomaly ← compares vs stationary
    │               │       └── resource_predictor ← historical resource data
    │               │
    │               └── module_output → UI rendering
    │
    ├── coordinator (agent orchestration)
    │       │
    │       ├── agent_registry (static capabilities)
    │       ├── agent_sessions (runtime sessions)
    │       └── recommendations → UI cards
    │
    └── UI (ratatui rendering)
            ├── layout (adaptive composition)
            ├── chat, agents_view, input
            ├── entropy_bar, agent_disagreement
            └── recommendations, permission, card
```

## Proposed Integration Plan with SuperInstance

### Phase 1: Bridge Layer
- Expose the `TerminalModule` trait as a SuperInstance handler interface
- Map `ModuleOutput` types to SuperInstance message types
- Bridge `TerminalEvent` to SuperInstance event bus

### Phase 2: Agent Federation
- Register intelligent-terminal agents in the SuperInstance capability registry
- Use `agent_registry.rs` profiles as the canonical agent metadata
- Bridge `coordinator.rs` recommendation flow to SuperInstance orchestration

### Phase 3: Analysis Pipeline
- Feed `command_markov` and `forecast` data into the SuperInstance MODEL layer
- Connect `error_hodge` decomposition to SuperInstance diagnostic pipelines
- Expose `spectral_dashboard` metrics as SuperInstance health indicators

### Phase 4: State Synchronization
- Use `memory_budget` LRU patterns for SuperInstance cache invalidation
- Sync `agent_sessions` state with SuperInstance pincherOS reflex layer
- Persist `transition_matrix` state via SuperInstance HARDCODE persistence

### Phase 5: UI Integration
- Port `entropy_bar` and `agent_disagreement` visualizations to SuperInstance dashboard
- Bridge `recommendations` cards to SuperInstance action cards
- Connect `command_popup` autocomplete to SuperInstance MODEL suggestions
