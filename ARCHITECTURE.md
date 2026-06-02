# Intelligent Terminal: Module System Architecture

> **Audience:** Engineers considering upstreaming this work into Windows Terminal / dev tools infrastructure.
> **Codebase:** `tools/wta/` — Rust TUI, `~14 KLOC`, the WTA (Windows Terminal Agent) client.
> **Status:** Design + partial implementation (math_analysis, griot_history modules shipped).

---

## Overview

The module system solves a hard constraint: a terminal must start fast and stay fast, even when it carries mathematical analysis engines, ML-adjacent computations, and persistent history models. Our answer is **zero-cost dormancy** — modules that do not exist until triggered, and cease to exist when disabled, with no runtime measurement overhead of any kind.

Two modules are currently implemented:

| Module | Feature flag | What it does |
|--------|-------------|--------------|
| `math_analysis` | `math-tools` | Markov chain command analysis, Hodge error decomposition, verification entropy tracking, spectral agent-graph metrics |
| `griot_history` | `griot-history` | Exponential-decay command memory, workflow pattern mining, context-aware alias suggestion, persistence barcode visualization |

---

## Principle 1: Zero-Cost Dormancy

Every module is **compile-time erased** when its feature flag is not set. This is not a runtime branch — there is no `if enabled { ... }` in the hot path. The code literally does not exist in the binary.

```
Cargo.toml feature flags
        │
        ▼
#[cfg(feature = "math-tools")]    ← entire subtree compiled away when off
pub mod math_analysis;
    │
    ├── command_markov.rs          ← nalgebra dependency pulled only when ON
    ├── error_hodge.rs             ← zero weight in binary when OFF
    ├── verification_entropy.rs
    └── spectral_dashboard.rs
```

The `mod.rs` files are the gatekeepers:

```rust
// src/math_analysis/mod.rs
#[cfg(feature = "math-tools")]
pub mod command_markov;
#[cfg(feature = "math-tools")]
pub mod error_hodge;
#[cfg(feature = "math-tools")]
pub mod spectral_dashboard;
#[cfg(feature = "math-tools")]
pub mod verification_entropy;

#[cfg(feature = "math-tools")]
pub use command_markov::{CommandMarkovChain, Anomaly};
#[cfg(feature = "math-tools")]
pub use error_hodge::{ErrorHodge, ErrorDecomposition};
// ...
```

```rust
// src/griot_history/mod.rs
#[cfg(feature = "griot-history")]
pub mod decay;
#[cfg(feature = "griot-history")]
pub mod pattern;
#[cfg(feature = "griot-history")]
pub mod adinkra;
#[cfg(feature = "griot-history")]
pub mod persistence;
```

**What this means for upstreaming:** A Windows Terminal build that does not pass `--features math-tools` has identical binary size and startup time to one without these modules in the tree at all. The feature flags are the upstream adoption dial.

---

## Principle 2: Context-Triggered Activation

Modules do not initialize on process start. They initialize **when the context that requires them first appears**. Each module type has a defined trigger:

```
Terminal event stream
        │
        ├── Command executed ──────────────────► GriotModule.record()
        │                                             (first call initializes DecayModel)
        │
        ├── Error detected (exit ≠ 0) ──────────► HodgeModule.decompose()
        │                                             (stateless — no init needed)
        │
        ├── File edited ──────────────────────────► VerificationEntropy.record_edit()
        │                                             (cumulative counter)
        │
        ├── Test command run ─────────────────────► VerificationEntropy.record_test()
        │                                             (counter reset + discharge event)
        │
        ├── Agent session added/removed ──────────► SpectralDashboard.graph mutation
        │                                             → invalidate_cache()
        │                                             → lazy recompute on next tick
        │
        └── Agent response parsed ────────────────► CommandMarkovChain.record_transition()
                                                      (stationary dist cached, recomputed
                                                       only when chain mutates)
```

The griot history `GriotAnalysis::analyze()` entry point demonstrates this pattern — it builds and populates all sub-models in a single call triggered by an external event, not on startup:

```rust
pub fn analyze(commands: &[(String, u64)], project_files: &[&str]) -> Self {
    let mut decay_model = DecayModel::new();
    for (cmd, ts) in commands {
        decay_model.record(cmd.clone(), *ts);      // lazy population
    }
    let pattern_miner = PatternMiner::from_commands(commands);
    let project_context = AdinkraCompressor::detect_project(project_files);
    let barcode = PersistenceBarcode::from_model(&decay_model);
    GriotAnalysis { decay_model, patterns, plateaus, project_context, barcode }
}
```

---

## Principle 3: Feature Gating (Two Tiers)

The system has two independent gates:

```
Tier 1: Compile-time (Cargo features)           Tier 2: Runtime (config)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━           ━━━━━━━━━━━━━━━━━━━━━━━━
                                                 
  cargo build                                    [modules]
  ↓ no features → no module code               conservation.enabled = true
                                                sheaf.enabled = false
  cargo build --features math-tools             spectral.enabled = true
  ↓ math analysis compiled in                   griot.enabled = true
                                                entropy_bar.enabled = true
  cargo build --features griot-history
  ↓ griot history compiled in                   Modules that are compiled in
                                                 but runtime-disabled:
  cargo build --features all                    → activate() never called
  ↓ all modules compiled in                     → no event loop overhead
                                                → config can re-enable live
```

This gives operators two knobs: one for binary footprint (compile-time), one for per-deployment behavior (runtime config). A Windows Terminal release build ships with `--features all` and lets users toggle via `~/.config/intelligent-terminal/modules.toml`.

---

## Principle 4: Module Lifecycle

Every module follows a three-phase lifecycle. The trait:

```rust
#[async_trait]
pub trait TerminalModule: Send + Sync {
    fn id(&self) -> &str;

    // Phase 1: Called once on first trigger.
    // Receives the event bus sender and read access to history/pane output.
    async fn activate(&mut self, ctx: ModuleContext) -> Result<()>;

    // Phase 2: Called on App::Tick for poll-driven modules.
    // Default no-op — most modules are event-driven.
    async fn tick(&mut self) -> Result<()> { Ok(()) }

    // Phase 3: Called on quit or runtime disable.
    // Flush state, release resources.
    async fn deactivate(&mut self) -> Result<()>;
}
```

The lifecycle state machine:

```
  ┌──────────────────────────────────────────────────────────────┐
  │                    MODULE LIFECYCLE FSM                       │
  │                                                               │
  │  ┌──────────┐  first trigger  ┌──────────┐                   │
  │  │ Dormant  │────────────────►│  Active  │◄──┐               │
  │  │(no alloc)│                 │          │   │ re-enable      │
  │  └──────────┘                 └────┬─────┘   │               │
  │       ▲                           │           │               │
  │       │   runtime disable         ▼           │               │
  │       │ ◄─────────────── ┌──────────────┐    │               │
  │       │                  │  Deactivated  │────┘               │
  │       │                  │  (state kept) │                    │
  │       │                  └──────────────┘                     │
  │       │                                                        │
  │  (compile-time off: this diagram does not exist in binary)    │
  └──────────────────────────────────────────────────────────────┘
```

### SpectralDashboard: Tick-Driven Lifecycle Example

The `SpectralDashboard` is the most explicit lifecycle implementor. It manages expensive eigenvalue computation behind a tick counter:

```rust
pub struct SpectralDashboard {
    pub graph: AgentGraph,
    pub last_fiedler: Option<f64>,
    pub last_cheeger: Option<f64>,
    pub last_mixing_time: Option<usize>,
    ticks_since_update: u64,
    recompute_interval: u64,       // default: 10 ticks
}

impl SpectralDashboard {
    pub fn tick(&mut self) {
        self.ticks_since_update += 1;
        if self.ticks_since_update >= self.recompute_interval {
            self.recompute();        // eigenvalue computation deferred here
            self.ticks_since_update = 0;
        }
    }

    pub fn recompute(&mut self) {
        if self.graph.num_nodes() >= 2 {
            self.last_fiedler = self.graph.fiedler_value();      // O(n²) — capped
            self.last_cheeger = self.graph.cheeger_constant();   // sweep cut
            self.last_mixing_time = self.graph.mixing_time();    // τ ≈ 1/λ₂
        }
    }
}
```

The `dirty` flag on `AgentGraph` is a second-tier guard: even if `tick()` fires, the eigenvalue recomputation is skipped if the graph has not changed since last computation.

```
  AgentGraph state machine:
  
  [add_node / add_edge / remove_node]
           │
           ▼
      dirty = true
      cached_fiedler = None
      cached_cheeger = None
      cached_mixing_time = None
           │
           ▼ (on next recompute())
      rebuild_matrices()
           │
           ├── compute eigenvalues (power iteration + LU solve)
           ├── compute Cheeger sweep
           └── cache results + dirty = false
                    │
                    ▼
               Subsequent fiedler_value() / cheeger_constant() calls
               return cached values — O(1) reads
```

---

## Principle 5: Memory Budgets

Every module declares a hard ceiling on its memory footprint. These are not soft limits or runtime checks — they are structural invariants baked into the data layout.

### CommandMarkovChain — Fixed-Size Transition Matrix

```
CommandMarkovChain memory layout:

  max_commands = 512 (default)
  ┌─────────────────────────────────────────────┐
  │ counts: Vec<u64>   → 512 × 512 × 8 = 2 MB  │  ← pre-allocated flat Vec
  │ command_index: HashMap<String, usize>        │  ← grows until max_commands
  │ index_command: Vec<String>                   │  ← len ≤ max_commands
  │ cached_stationary: Option<Vec<f64>>          │  ← len ≤ max_commands
  └─────────────────────────────────────────────┘

  Exceeding max_commands panics at insertion — deliberate:
  assert!(idx < self.max_commands, "exceeded max_commands ({})", ...);

  To tune: CommandMarkovChain::with_max_commands(256)  → 512 KB
           CommandMarkovChain::with_max_commands(512)  → 2 MB   (default)
           CommandMarkovChain::with_max_commands(1024) → 8 MB
```

### ErrorHodge — Sliding Window History

```rust
const MAX_HISTORY: usize = 20;

pub fn push_command(&mut self, command: String) {
    self.recent_commands.push(command);
    if self.recent_commands.len() > MAX_HISTORY {
        self.recent_commands.remove(0);   // O(n) but n ≤ 20, acceptable
    }
}
```

`ErrorHodge` is stateless across sessions — it holds only the 20-command sliding window needed to compute prior mismatch scores. No unbounded growth.

### SpectralDashboard — Graph Node Cap

```
AgentGraph is designed for the multi-agent scenario, not massive graphs:

  Typical: 2-6 agents (Copilot, Claude, Codex, Gemini + local)
  Cap enforced by: never adding nodes beyond active agent sessions
  Eigenvalue computation: O(n²) power iteration, n capped by agent count

  If n < 2: fiedler_value() and cheeger_constant() return None immediately
            → no matrix allocation, no computation
```

### VerificationEntropy — Counters Only

```
VerificationEntropy is O(1) space, always:

  edits_since_last_test: u64      ← reset to 0 on test
  total_lines_edited: u64
  total_tests_run: u64
  alpha, lines_per_test_unit,
  medium/high/critical thresholds: f64 × 5

  Total: ~56 bytes.
  Entropy formula: E = 1 - exp(-α · edits_since_last_test / L)
```

### DecayModel — Soft Ring Buffer

The griot history `DecayModel` grows with command history but is designed for pruning:

```
DecayModel layout:
  records: Vec<CommandRecord>           ← one record per command execution
  retelling_counts: HashMap<String, u32> ← one entry per unique command
  reference_time: u64

Retelling count update: O(n) scan on every record().
This is acceptable because the scan enables the core invariant:
  "Every time you re-run a command, ALL prior instances are strengthened."

For production: prune records older than (reference_time - 30 * 86400) seconds
                to bound growth. The decay formula makes old records irrelevant
                anyway (strength approaches 0 after ~30 days with default λ).
```

---

## Module Internals Reference

### math_analysis: Four Sub-Modules

```
math_analysis/
├── command_markov.rs   ← CommandMarkovChain
│     Purpose: Track command-to-command transitions as a Markov chain.
│     Trigger: Record each executed command.
│     Output:  stationary_distribution(), mixing_time(), check_anomaly()
│     Math:    Power method on transpose for stationary dist (πP = π).
│              Shift-invert for mixing time via TV distance.
│
├── error_hodge.rs      ← ErrorHodge, ErrorDecomposition
│     Purpose: Decompose any error into evidence / coherence / prior_mismatch.
│     Trigger: Any failed command (non-zero exit code or substantive stderr).
│     Output:  ErrorDecomposition { evidence, coherence, prior_mismatch, dominance }
│     Math:    Scoring heuristics + dominance classification (Hodge-inspired
│              orthogonal decomposition metaphor; not literal Hodge theory).
│
├── verification_entropy.rs ← VerificationEntropy, VerificationEvent
│     Purpose: Track edit-to-test ratio as thermodynamic entropy.
│     Trigger: Every file edit (+lines) and every test command (-entropy).
│     Output:  compute_entropy() ∈ [0,1], status_bar_label(), VerificationEvent
│     Math:    E = 1 - exp(-α · edits_since_last_test / L), clamped [0,1].
│
└── spectral_dashboard.rs ← SpectralDashboard, AgentGraph
      Purpose: Measure algebraic connectivity of the multi-agent network.
      Trigger: Agent sessions added/removed; periodic tick.
      Output:  fiedler_value(), cheeger_constant(), mixing_time(), status_bar_indicator()
      Math:    Laplacian L = D - A; λ₂ via power iteration + LU shift-invert;
               Cheeger sweep cut on Fiedler eigenvector.
```

### griot_history: Four Sub-Modules + Orchestrator

```
griot_history/
├── decay.rs          ← DecayModel, CommandRecord, RetellingStrength
│     Purpose: Model command memory with exponential decay + retelling boost.
│     Core law: strength(t) = exp(-λ · age) · (1 + retelling_count · 0.3)
│     Half-life: 5 days (432,000 s). λ = ln(2) / half_life.
│     Retelling: every re-run of a command boosts ALL prior instances.
│
├── pattern.rs        ← PatternMiner, WorkflowPattern, LearningPlateau
│     Purpose: Detect repeated command subsequences + skill plateaus.
│     Pattern mining: frequency counting over sliding windows, length 2-5.
│     Plateau detection: diversity = |unique cmds| / window_size < 0.4 → plateau.
│     Renormalization: take top-N most frequent patterns as "coarse-grained" view.
│
├── adinkra.rs        ← AdinkraCompressor, ProjectContext, AliasSuggestion
│     Purpose: Detect project type from filesystem, suggest context-aware aliases.
│     Trigger files: Cargo.toml → Rust, package.json → Node, go.mod → Go, etc.
│     Adoption tracking: alias → usage_count; rank_suggestions() by adoption.
│     Cultural metaphor: adinkra symbols compress complex concepts — aliases do too.
│
├── persistence.rs    ← PersistenceBarcode, PersistenceDiagram
│     Purpose: Visualize which commands survive temporal decay (ASCII art).
│     Barcode: each slot maps strength to █▓▒░·; renders at any terminal width.
│     Diagram: birth/death plot per unique command showing persistence lifetime.
│
└── mod.rs            ← GriotAnalysis (orchestrator)
      Purpose: Top-level entry point that assembles all sub-modules.
      Usage:   GriotAnalysis::analyze(&commands, &project_files)
               → GriotAnalysis { decay_model, patterns, plateaus,
                                 project_context, barcode }
```

---

## Data Flow Diagrams

### math_analysis Data Flow

```
  ┌─────────────────────────────────────────────────────────────────┐
  │                    math_analysis data flow                       │
  │                                                                   │
  │  Input events          Module              Output                 │
  │  ─────────────         ──────              ──────                 │
  │                                                                   │
  │  prev_cmd, next_cmd ──►CommandMarkovChain──► stationary dist      │
  │                         (transition count)   mixing time          │
  │                         (power method)       Anomaly?             │
  │                                                                   │
  │  exit_code,         ──►ErrorHodge        ──► ErrorDecomposition   │
  │  stderr, history        (scoring + dom.)     evidence/coherence/  │
  │                                              prior_mismatch       │
  │                                              "70% prior mismatch" │
  │                                                                   │
  │  file_save(lines)   ──►VerificationEntropy─► EntropyLevel         │
  │  test_run()             (E = 1-exp(-α·n/L))  VerificationEvent    │
  │                                              status bar label      │
  │                                                                   │
  │  add_agent(id)      ──►AgentGraph         ──► Fiedler λ₂          │
  │  add_edge(w)            (Laplacian L=D-A)    Cheeger h             │
  │  tick()                 (power + LU)         mixing time t         │
  │                         (dirty flag cache)   "λ₂=0.34 h=0.21 t=3" │
  └─────────────────────────────────────────────────────────────────┘
```

### griot_history Data Flow

```
  ┌─────────────────────────────────────────────────────────────────┐
  │                   griot_history data flow                        │
  │                                                                   │
  │  Input                  Module              Output                │
  │  ─────                  ──────              ──────                │
  │                                                                   │
  │  command + timestamp ──►DecayModel        ──► strength(t)         │
  │                         λ = ln2/432000s       retelling boost     │
  │                         retelling_count++     persisting_commands │
  │                         boost ALL priors                          │
  │                                                                   │
  │  command sequence    ──►PatternMiner      ──► WorkflowPattern[]   │
  │                         (freq count       ──► LearningPlateau[]   │
  │                          len 2-5,             "cargo build→test   │
  │                          merge overlaps)       runs 47×"          │
  │                                                                   │
  │  project files list  ──►AdinkraCompressor ──► ProjectContext      │
  │                         (Cargo.toml?           suggested aliases  │
  │                          package.json?          {cb→cargo build,  │
  │                          go.mod? ...)            ct→cargo test}   │
  │                                                                   │
  │  DecayModel          ──►PersistenceBarcode──► ASCII barcode       │
  │                         (strength→char)        "█▓▓░··█████"      │
  │                                                PersistenceDiagram │
  └─────────────────────────────────────────────────────────────────┘
```

---

## Integration with App Event Loop

The modules plug into the App's existing `AppEvent` dispatch. New event variants are feature-gated so they compile away when disabled:

```rust
// In the AppEvent enum — conceptual sketch:
pub enum AppEvent {
    // Existing events (always present):
    Key(KeyEvent),
    Tick,
    AgentMessageChunk { tab_id, chunk },
    WtEvent { method, pane_id, params },

    // Feature-gated additions:
    #[cfg(feature = "math-tools")]
    MathComputed { tab_id: String, operation_id: u64, result: String },

    #[cfg(feature = "griot-history")]
    GriotAnalysisReady { tab_id: String, analysis: Box<GriotAnalysis> },
}
```

The App struct carries the module state, also feature-gated:

```rust
pub struct App {
    // ... existing ~160 fields ...

    #[cfg(feature = "math-tools")]
    pub math_state: Option<MathState>,       // None until first trigger

    #[cfg(feature = "griot-history")]
    pub griot_state: Option<GriotState>,     // None until first trigger
}
```

---

## Adding a New Module: Checklist

For a Microsoft engineer wanting to add a new module:

```
1. Cargo.toml
   ─────────
   [features]
   my-module = ["dep:optional-crate"]    ← add optional dependency

2. src/my_module/mod.rs
   ────────────────────
   #[cfg(feature = "my-module")]         ← gate the entire tree
   pub mod core;
   #[cfg(feature = "my-module")]
   pub use core::MyModule;

3. Memory budget declaration
   ─────────────────────────
   const MAX_ITEMS: usize = 512;         ← declare hard ceiling
   struct MyModule {
       items: Vec<Item>,                 ← grows to MAX_ITEMS only
       cache: Option<CachedResult>,      ← invalidated on mutation
       dirty: bool,
   }

4. Trigger definition
   ───────────────────
   // Document: what AppEvent causes this module to activate?
   // Is it event-driven or tick-driven?
   // Does it need history on first activation, or start fresh?

5. Lifecycle implementation
   ─────────────────────────
   impl TerminalModule for MyModule {
       async fn activate(&mut self, ctx: ModuleContext) -> Result<()> { ... }
       async fn tick(&mut self) -> Result<()> { ... }   // only if tick-driven
       async fn deactivate(&mut self) -> Result<()> { ... }
   }

6. AppEvent variants (feature-gated)
   ──────────────────────────────────
   #[cfg(feature = "my-module")]
   MyModuleEvent { ... },

7. Handle in App::handle_event (feature-gated)
   ─────────────────────────────────────────────
   #[cfg(feature = "my-module")]
   AppEvent::MyModuleEvent { .. } => { ... },
```

---

## Performance Guarantees

| Property | Mechanism | Guaranteed bound |
|----------|-----------|-----------------|
| Binary size when disabled | `#[cfg(feature)]` | 0 bytes added |
| Startup overhead when disabled | Compile-time erasure | 0 ns |
| Hot-path overhead when active | Event-driven, not polled | Only fires on trigger event |
| Eigenvalue computation | Tick-gated (`recompute_interval`), cached (`dirty` flag) | At most once per N ticks, O(1) on cache hit |
| Markov chain stationarity | `cached_stationary`, invalidated on mutation | O(n²) once, O(n) thereafter |
| Memory per module | Hard ceiling constants | CommandMarkovChain: 2 MB max; ErrorHodge: O(1); VerificationEntropy: 56 bytes; DecayModel: O(history_size) |
| No external calls | All math is pure local computation | No network, no subprocess, no API |

---

## Cargo Feature Summary

```toml
[features]
default = []                          # nothing enabled; bare terminal

# Individual modules:
math-tools    = ["dep:nalgebra", "dep:serde_json"]
griot-history = []                    # pure Rust, no new deps

# Convenience bundles:
all = ["math-tools", "griot-history"]
```

Building the full-featured binary:
```
cargo build --release --features all
```

Building the minimal binary (no modules):
```
cargo build --release
```

Both produce a correct, fully functional terminal. The only difference is capability surface.

---

## Design Decisions Worth Preserving

1. **Feature flags over runtime flags for code removal.** Runtime `if enabled` branches still execute the branch prediction miss and keep dead code in the binary. `#[cfg]` eliminates both. Use runtime flags only for behavior that is already compiled in.

2. **Dirty-flag caching over eager recomputation.** Eigenvalue computation is O(n²). The `dirty` flag ensures the expensive path runs only when the input has changed, not on every read. This pattern (used in `AgentGraph`, `CommandMarkovChain`) scales to any expensive derived value.

3. **Hard memory ceilings over soft limits.** `CommandMarkovChain::with_max_commands(n)` panics rather than silently growing. In a terminal that runs for weeks, silent growth leads to OOM. Explicit ceilings surface the tradeoff at configuration time.

4. **Stateless modules where possible.** `ErrorHodge::decompose()` takes all inputs as arguments and returns a value. No global state, no background task, no channel. When modules must accumulate state (`DecayModel`, `VerificationEntropy`), the state is owned by the module instance, not shared.

5. **Module isolation.** Each module owns its state. No cross-module shared mutable state. Modules communicate through `AppEvent` variants, the same bus used by the rest of the terminal. This makes modules individually testable and individually removable.
