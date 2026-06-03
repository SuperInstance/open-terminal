# The Terminal as Universal Harness

> **Audience:** Architects, integrators, and anyone asking "how does a terminal become a
> dependency of every repo in the ecosystem?"
>
> **Prerequisite:** [THREE_LAYER_ARCHITECTURE.md](../THREE_LAYER_ARCHITECTURE.md) (the concentric
> Natural → Fluid → Machine model) and [ARCHITECTURE.md](../ARCHITECTURE.md) (module system
> zero-cost dormancy).

---

## The Backbone

The seamless addition of any repo into the terminal comes from **one invariant** that every
contributing repo must satisfy:

> **Every repository is a compiled ontology, proves one theorem, serves one invariant,
> and exports one harness API.**

This is not a suggestion. It is the contract that makes the universal harness possible.
Without it, every integration is a bespoke adapter. With it, every integration is a
one-line `Cargo.toml` dependency.

### The Invariant in Detail

| Property | Meaning | Why it matters |
|----------|---------|----------------|
| **Compiled ontology** | The repo's types and functions model a coherent domain (spectral graph theory, sheaf cohomology, free probability, etc.) | The terminal imports ontology, not ad-hoc computation. Cross-module reasoning works because every module speaks a consistent domain language. |
| **Proves one theorem** | The crate has at least one property-based test or algebraic invariant that must hold for all valid inputs | Every module is self-certifying. The terminal's confidence in cross-module composition derives from each module's proved invariant. |
| **Serves one invariant** | The public API is minimal — one or two entry points — and the invariant is the test the terminal can run at startup to verify the module is healthy | The terminal can check health of any loaded module without understanding its internals. |
| **Exports one harness API** | A standard `{Module}Harness` struct with `{ init(), check_health(), describe(), shutdown() }` — the terminal's generic Module trait | The terminal treats every module identically. The `Harness` trait is the universal adapter. |

```rust
/// Every module in the terminal's dependency graph implements this trait.
pub trait Harness {
    /// Identity and version.
    fn describe(&self) -> ModuleInfo;
    /// Prove the invariant holds right now.
    fn check_health(&self) -> Result<HealthReport, HealthError>;
    /// Initialize with a context reference (terminal state, ACP client, etc.).
    fn init(&mut self, ctx: &HarnessContext) -> Result<(), InitError>;
    /// Gracefully tear down.
    fn shutdown(&mut self) -> Result<(), ShutdownError>;
}
```

Because every module is a `dyn Harness`, the terminal can load, health-check, and compose
any set of modules without knowing their math. **The terminal's job is orchestration and
presentation, not implementation.**

---

## Three Harness Layers

The harness model is not one layer — it is three, each with a different scope, frequency
of change, and integration mechanism.

### 1. Metal Library Harness — 12 Rust Crates

The **Metal Library Harness** is the innermost (Machine) layer: pure Rust crates that
the terminal optionally depends on. Each is an independent git repository, independently
versioned and documented.

| Crate | Domain | Theorem (Health Invariant) |
|-------|--------|---------------------------|
| `cst-rs` | Context-sensitive sheaf theory | `pullback ∘ pushforward = id` on acyclic cover |
| `sheaf-rs` | Sheaf cohomology for agent coordination | Cohomology `H¹(X, ℱ) = 0` for contractible spaces |
| `hodge-rs` | Hodge belief propagation | `Δ = dδ + δd`; harmonic component error `< ε` |
| `spectral-graph-rs` | Spectral graph theory | Fiedler eigenvalue `λ₂` is second-smallest |
| `renorm-rs` | Renormalization group flows | RG step `R` satisfies semigroup `R(g) ∘ R(g') = R(gg')` |
| `west-african-rs` | West African geometry (meditation, sand-counting, brass-casting) | Conservation law `∇·J + ∂ρ/∂t = 0` for cultural flows |
| `free-prob-rs` | Free probability (Voiculescu) | `φ(ab) = φ(a)φ(b)` for freely independent variables |
| `ergodic-rs` | Ergodic transport | Time average = space average within `ε` after mixing time |
| `CSF-rs` | Conservation spectral flows | Spectral flow is divergence-free |
| `evolving-sheaf-rs` | Time-varying sheaf structures | Sheaf evolution `ℱ_t` is a functor `Time → Sheaves(X)` |
| `persistent-sheaf-rs` | Persistent sheaf homology | Persistence module satisfies `V_a → V_b → V_c` exactness |
| `integration-rs` | Higher-categorical integration | Integral is a natural transformation `∫: Hⁿ(X) → ℝ` |

**Wiring.** The terminal does not call these libraries directly from the TUI. Instead,
each crate is wrapped by a thin adapter module:

```toml
# tools/wta/Cargo.toml
[dependencies]
spectral-graph-rs = { git = "https://github.com/SuperInstance/spectral-graph-rs", optional = true }
hodge-rs          = { git = "https://github.com/SuperInstance/hodge-rs", optional = true }
# ... 10 more
```

```rust
// tools/wta/src/harness/spectral.rs
#[cfg(feature = "metal-libs")]
pub struct SpectralHarness {
    inner: spectral_graph_rs::GraphEngine,
}

#[cfg(feature = "metal-libs")]
impl Harness for SpectralHarness {
    fn describe(&self) -> ModuleInfo {
        ModuleInfo { name: "spectral-graph", version: env!("CARGO_PKG_VERSION"), domain: "Spectral graph theory" }
    }
    fn check_health(&self) -> Result<HealthReport, HealthError> {
        let λ₂ = self.inner.fiedler_eigenvalue(&self.inner.test_graph())?;
        // The theorem: λ₂ is the second-smallest eigenvalue of the Laplacian.
        // We verify it's not the smallest and not the third.
        assert!(λ₂ > self.inner.smallest_eigenvalue()? && λ₂ < self.inner.third_eigenvalue()?);
        Ok(HealthReport { healthy: true, metric: λ₂ })
    }
    // ...
}
```

**Key principle:** The terminal wires their math to the UI. It does not implement any
of it. If the math is wrong, the fix is in the crate, not the terminal.

### 2. PincherOS Harness — The Reflex Engine

The **PincherOS Harness** is the middle (Fluid) layer: a lifelong learning system that
lives inside the terminal and adapts to the user's workflow in real time. It is the
"reflex engine."

#### Command Prediction → Reflex Compilation

Every command you type is observed by PincherOS. Over time, PincherOS builds a Markov
model of your shell sessions. When it predicts your next command with confidence above
a threshold, it **compiles the prediction into a reflex** — a hot-key or auto-completion
that fires without waiting for you to finish typing.

```
Observation: "git add ." → "git commit -m" → "git push"
  ↓
After N repetitions, PincherOS builds a transition tuple
  ↓
PincherOS compiles a reflex: [git add] → auto-tab to "git commit -m"
  ↓
After M repetitions, PincherOS promotes to an alias: `gacp → git add . && git commit -m && git push`
```

The reflex compilation pipeline:

```
User types ─→ Pattern Buffer ─→ Markov Chain ─→ Confidence Gate (>0.85) ─→ Reflex Compiler ─→ Terminal Binding
                                                                                │
                                                                                └─→ Alias (for repeated exact matches)
```

#### Error Patterns → Learn

When a command fails, PincherOS absorbs the error and updates its transition model.
Over time, it learns not just what you do, but what you **don't** want to do.

```
"git brunch" → [ERROR: unknown command]
  ↓
PincherOS: reads error, notes "brunch" is adjacent to "branch" in edit-distance
  ↓
PincherOS adds a negative weight: git[brunch] → -∞
  ↓
Next time you type "git bru", PincherOS suggests "branch" and demotes "brunch"
```

#### The Terminal as PincherOS's Incubator

PincherOS is not a finished product. It is a learning system that lives inside the
terminal because the terminal is where the learning happens. Every terminal session
is a training run. Every command is a data point. Every error is a label.

The terminal is where PincherOS lives **while it learns**. When PincherOS graduates
to a standalone agent, the terminal's job is done — but until then, the terminal
is both PincherOS's home and its proving ground.

### 3. Trending Repo Harness — Clone, Analyze, Decompose, Absorb

The **Trending Repo Harness** is the outermost (Natural) layer: a pipeline that turns
any GitHub repo into a potential terminal module.

#### The Pipeline

```
Trending repo detected (GitHub API / user request)
  │
  ├─1. Clone ──→ git clone into /tmp/harness/{repo}/
  │
  ├─2. Analyze ──→
  │     • Language detection (Rust, Python, TypeScript, etc.)
  │     • Dependency tree (what does this repo depend on?)
  │     • Entry point detection (main.rs, __init__.py, index.ts)
  │     • Test framework detection
  │     • Module boundary inference (pub / export / module declarations)
  │     • README intent extraction (what problem does this solve?)
  │
  ├─3. Decompose ──→
  │     • Hodge decomposition: separate the repo into gradient (features),
  │       curl (concurrency/state), and harmonic (stable API) components
  │     • Spectral analysis: find the eigenvector structure of the repo's
  │       internal dependency graph
  │     • Sheaf cohomology: identify which modules must agree on shared data
  │
  └─4. Absorb ──→
        • Generate a thin Harness adapter (if the repo is Rust)
        • Symlink to a local cargo workspace member
        • Register the module in the terminal's module registry
        • Health-check the new module against its own tests
        • Report "absorbed" status to the user
```

The output of the pipeline is always the same: a `Harness` implementation, registered
in the terminal's module graph, with a health check and a description.

#### What "Absorb" Means

When a repo is absorbed, it is not vendored. The terminal does not copy the code. It
creates a **thin shell** — a minimal adapter that satisfies the `Harness` trait — and leaves
the actual code at its canonical location. The terminal depends on the repo as a git
dependency from that point forward.

If the repo has no Rust interface (Python, JS, etc.), the shell bridges via subprocess
or WASM, with the terminal providing the glue. The invariant contract still holds:
the remote repo must still "prove one theorem" and "serve one invariant" — even if
that invariant is checked by running its test suite via subprocess.

---

## Architecture Invariants

These invariants are not guidelines. They are the rules that make the three-layer harness
composable, safe, and upgradable.

### 1. Every Module Is Feature-Gated

```toml
[features]
metal-libs  = ["spectral-graph-rs", "hodge-rs", ...]
pinceros    = ["pinceros-engine"]
harvester   = ["git2", "tokei", "serde_yaml"]
```

**Zero cost when disabled.** If you don't enable `metal-libs`, the spectral and Hodge
modules compile to nothing — not even a stub. `#[cfg(feature = ...)]` is the gate.

### 2. Every Module Has a Shell Fallback

Before any module is marked stable, its **shell fallback** must ship. The fallback is
a pure-shell implementation of the module's core function — no compiled code, no
feature gate, just shell commands that approximate what the module does.

| Module | Shell fallback |
|--------|---------------|
| Spectral graph | `pip install networkx && python3 -c "import networkx as nx; ..."` |
| Hodge | `python3 -c "import numpy as np; from scipy.sparse.linalg import ..."` |
| Griot-history | `cat ~/.bash_history \| sort \| uniq -c \| sort -rn \| head -20` |

**Why?** Because the terminal must never fail to do its job because a feature is
disabled. The shell fallback ensures the function exists — slower, dumber, but always
available. The compiled module is an upgrade, not a prerequisite.

**The fallback ships before the module is marked stable.** No module enters the stable
ring without a shell fallback that has been tested in the wild.

### 3. The Terminal Re-Implements Nothing

The terminal's Rust source tree has exactly one job: orchestration, UI, and glue. Every
mathematical algorithm, every non-trivial data structure, every ML-adjacent computation
lives in an external crate or a subprocess.

```
terminal/src/
  ├── harness/       ← Adapters for external crates (Harness trait)
  ├── tui/           ← Terminal UI (ratatui, crossterm)
  ├── acp/           ← ACP/MCP tool server
  ├── config/        ← User config (JSON, TOML, env)
  ├── pinceros/      ← PincherOS engine (fluid layer)
  └── main.rs        ← Entry point, wires everything together

  NO:
  └── spectral/      ← Nope, that lives in spectral-graph-rs
  └── hodge/         ← Nope, that lives in hodge-rs
  └── sheaf/         ← Nope, that lives in sheaf-rs
```

**If you're writing a math function in the terminal crate, you're doing it wrong.**
Move it to a metal library crate, publish it, and depend on it.

### 4. The Three-Layer Model Is Concentric

Reiterating the model from [THREE_LAYER_ARCHITECTURE.md](../THREE_LAYER_ARCHITECTURE.md)
in the context of the harness:

```
NATURAL (outer) — Trending Repo Harness: clone, analyze, decompose, absorb
  │
  │  Intent boundary (Natural → Fluid): vague human desire → concrete action plan
  │
FLUID (middle)  — PincherOS Harness: learn, predict, reflex, adapt
  │
  │  Expression boundary (Fluid → Machine): action plan → compiled math
  │
MACHINE (inner) — Metal Library Harness: spectral, hodge, sheaf, renorm, ...
```

Each harness layer maps to one concentric ring. No layer can skip a boundary. No
layer "falls back" to an outer layer — the outer layer is the **interface**, not a
fallback.

### 5. Every Boundary Is a Dual Aspect Functor

A **dual aspect functor** translates both structure and meaning in both directions.
This is not a metaphor — it is a design constraint on every cross-boundary API.

**Natural → Fluid (Intent boundary):**

```
Structure:  [string "check agent health"] → [Action::Spectral { node: "all" }]
Meaning:    "I want diagnostics" → "Run spectral analysis on all agents"
```

**Fluid → Machine (Expression boundary):**

```
Structure:  [Action::Spectral { node: "all" }] → [SpectralParams { k: 3, ε: 0.01 }]
Meaning:    "Run spectral analysis" → "Compute top-3 eigenpairs with 1% tolerance"
```

**Machine → Fluid (Result boundary):**

```
Structure:  [λ₁=0.0, λ₂=0.34, λ₃=1.2] → [SpectralResult { components: [...], harmonics: [...] }]
Meaning:    Raw eigenvalues → "The graph has one disconnected component and a bottleneck"
```

**Fluid → Natural (Explanation boundary):**

```
Structure:  [SpectralResult { ... }] → "🔍 The agent graph has a bottleneck at node C"
Meaning:    Structured result → Human-readable explanation
```

Dual aspect functors ensure that:
- **Every boundary is symmetric.** You can traverse it in either direction.
- **No information is lost.** The translation preserves semantic content even as it
  changes representation.
- **Boundaries are testable in isolation.** You can test the functor without the
  adjacent layer.

---

## The Loop That Proves Itself

The harness architecture is not static. It is a **closed loop** that improves itself
through use. Every interaction reinforces the system's ability to predict, decompose,
and absorb.

```
                    ┌─────────────────────────────────────┐
                    │           YOU TYPE                   │
                    │  (command, script, alias, query)     │
                    └────────────┬────────────────────────┘
                                 │
                                 ▼
                    ┌─────────────────────────────┐
                    │   TERMINAL (PincherOS)       │
                    │  • Observes the command      │
                    │  • Updates Markov model      │
                    │  • Checks for error patterns │
                    └────────────┬─────────────────┘
                                 │
                    ┌────────────▼──────────────┐
                    │   REFLEX COMPILATION       │
                    │  • Pattern matched?        │
                    │  • Confidence > threshold? │
                    │  • Compile hot-key / alias │
                    └────────────┬───────────────┘
                                 │
                    ┌────────────▼──────────────┐
                    │   SKILL PLATEAU?           │
                    │  (Same commands →          │
                    │   diminishing returns)     │
                    └────────────┬───────────────┘
                                 │ yes
                                 ▼
                    ┌────────────────────────────┐
                    │   RENORMALIZATION STEP      │
                    │  • coarsen pattern grain    │
                    │  • merge sequences          │
                    │  • next plateau is higher   │
                    └────────────┬───────────────┘
                                 │
                                 ▼
                    ┌────────────────────────────┐
                    │   METAL LIBRARY CALL        │
                    │  • Spectral decomposition   │
                    │  • Hodge error separation   │
                    │  • Sheaf consistency check  │
                    └────────────┬───────────────┘
                                 │
                                 ▼
                    ┌────────────────────────────┐
                    │   YOU LEARN                 │
                    │  (see result, adapt)        │
                    └────────────┬───────────────┘
                                 │
                                 └──→ back to "YOU TYPE" ──→ loop continues
```

### What Each Arrow Means

| Arrow | Mechanism | Who owns it |
|-------|-----------|-------------|
| Command → PincherOS | `~/.bash_history` / `pyncher` event | Terminal (Fluid) |
| PincherOS → Reflex | Markov chain prediction | PincherOS Engine (Fluid) |
| Reflex → Renorm | Skill plateau detection | PincherOS Engine + Renorm Crate (Fluid → Machine) |
| Renorm → Metal Lib | Hodge / Spectral / Sheaf | Metal Library Harness (Machine) |
| Metal Lib → You | TUI output, status bar, notification | Terminal TUI (Natural) |

### The Three Clauses

1. **Every command compiles a better reflex (PincherOS).** The more you type, the
   better the predictions. The terminal is a lifelong learning system.
2. **Every error improves the decomposition (Hodge).** The Hodge harness separates
   error into gradient (user mistake), curl (concurrent conflict), and harmonic
   (system invariant violation). Over time, error patterns reveal the structure of
   your workflow.
3. **Every skill plateau accelerates the next RG step (renormalization).** When
   PincherOS detects that your command patterns have stabilized (no new transitions
   for N sessions), it triggers a renormalization step that coarsens the pattern
   grain. The next plateau is always higher than the last.

### The Bottleneck Is You

The pipeline's throughput is bounded by how fast you can type, read, and decide. The
machine layer is arbitrarily fast — spectral decompositions, Hodge calculations,
sheaf cohomology all complete in milliseconds. The terminal is the interface that
waits for you. Everything else is unbounded.

> **The terminal doesn't wait for you to ask. The terminal watches the math and
> speaks when the math says something is wrong. When the math can't decide, the
> shell waits. When the shell can't decide, you type. The fallback chain is never
> broken.**

---

## Closing

The Universal Harness architecture transforms the terminal from a passive text
interface into an active mathematical observatory. Every repo that satisfies the
invariant (compiled ontology, one theorem, one invariant, one harness API) plugs
in seamlessly. The three-layer model (Metal Library → PincherOS → Trending Repo)
ensures that every module has the right scope, the right durability, and the right
fallback.

The terminal is not an application. It is the harness that proves the loop.

---

## See Also

- [THREE_LAYER_ARCHITECTURE.md](../THREE_LAYER_ARCHITECTURE.md) — The concentric Natural →
  Fluid → Machine model
- [ARCHITECTURE.md](../ARCHITECTURE.md) — Module system zero-cost dormancy
- [METAL_LIBRARY_INTEGRATION.md](./METAL_LIBRARY_INTEGRATION.md) — Metal library crate
  integration details
- [CORRECTED_MODEL.md](./CORRECTED_MODEL.md) — Why the fallback view was wrong
