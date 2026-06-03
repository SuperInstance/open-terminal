# Three-Layer Architecture

> The corrected concentric model for the Intelligent Terminal + SuperInstance stack.
> This document replaces the old "shell-as-fallback" view with the proper three-ring model.

---

## Overview

The Intelligent Terminal stack is organized as three concentric layers. Each layer has a distinct responsibility, a distinct language, and distinct durability properties. The boundaries between layers are **dual aspect functors** — they translate both structure and meaning in both directions.

```
                         ┌─────────────────────┐
                         │       NATURAL        │  outermost ring
                         │   Shell Interface    │
                         │  interpreted by      │
                         │  humans & LLMs       │
                         │                      │
                         │  ┌───────────────┐   │
                         │  │     FLUID      │   │
                    intent │  │  (hot path)   │   │
                 boundary  │  │              │   │
               natural→fluid│ │  Adaptive     │   │
                         │  │  compiled-but- │   │
                         │  │  parameterized │   │
                         │  │  context-      │   │
                         │  │  sensitive     │   │
                         │  │                │   │
                         │  │  ┌──────────┐  │   │
                         │  │  │ MACHINE  │  │   │
                         │  │  │ (cold    │  │   │
                    expr   │  │  path)    │  │   │
                boundary   │  │          │  │   │
               fluid→machine││  Rust     │  │   │
                         │  │  Zero-cost │  │   │
                         │  │  Feature-  │  │   │
                         │  │  gated     │  │   │
                         │  │  Bit-      │  │   │
                         │  │  identical │  │   │
                         │  │  Slow-to-  │  │   │
                         │  │  change    │  │   │
                         │  │  └──────────┘  │   │
                         │  └───────────────┘   │
                         └─────────────────────┘
```

---

## The Three Layers

### NATURAL (Outermost Ring)

**The shell interface.** This is what the user sees and interacts with — commands, output, prompts, error messages, autocomplete suggestions, agent pane responses. Everything in this layer is **text interpreted by humans and LLMs**. It is imprecise, contextual, and expressive — exactly what a human interface should be.

- **Language:** Natural language, shell syntax, ASCII/Unicode text, terminal escape sequences
- **Durability:** Ephemeral — scrollback buffer only. No structural guarantees.
- **Boundary function:** Accepts natural/fluid expressions from the user, relays machine results back

**This is NOT the "fallback."** The old model incorrectly positioned the shell as a fallback when higher-level processing failed. In reality, the shell is the **primary interface** — the outermost ring through which all interaction flows. It doesn't "fall back" to anything. It's where you start.

### FLUID (Middle Ring — The Hot Path)

**The proper language in the transformation graph.** This layer is adaptive, compiled-but-parameterized, and context-sensitive. It is the "hot path" — where most runtime decisions are made. Modules in this layer:

- Accept natural intent from the shell and compile it into machine parameters
- Accept machine results from the core and extract human-meaningful summaries
- Maintain state, cache expensive computations, manage module lifecycles
- Adapt behavior based on context (current project, recent commands, agent availability)

- **Language:** Structured data, configuration, cached state, adaptive queries
- **Durability:** Session-persistent — state lives for the session, can be serialized to disk
- **Boundary functions:**
  - **Semantic boundary (fluid → natural):** Extracts meaning from machine output and renders it as explanation
  - **Compilation boundary (fluid → machine):** Compares natural intent into machine-executable operations

### MACHINE (Innermost Ring — The Cold Path / Fixed Point)

**The compiled core.** Pure Rust. Feature-gated, zero-cost abstractions. Bit-identical computations that produce the same result every time given the same input. This layer changes slowly and deliberately.

- **Language:** Rust, nalgebra matrices, fixed-size buffers, hard memory ceilings
- **Durability:** Permanent — baked into the binary at compile time
- **Characteristics:**
  - Zero runtime overhead when feature-disabled (`#[cfg]` erasure)
  - No external calls (no network, no subprocess, no API requests)
  - Hard memory ceilings (panics on overflow — not silent growth)
  - Pure math — eigenvalue decomposition, Markov chain analysis, spectral graph metrics

---

## The Dual Aspect Functors

Each boundary between layers is a **dual aspect functor** — it translates in both directions, preserving structural pattern while changing representation.

### Natural → Fluid: Intent Compilation

```
User says: "find bottleneck"
          ↓
Fluid: IntentMatcher.select("bottleneck")
          → matches "spectral analysis" trigger
          → picks Laplacian eigenvalue computation
          → dispatches to Machine
```

The functor takes a vague natural-language prompt and compiles it into a concrete fluid intent. This is context-sensitive — "bottleneck" in a multi-agent context means Fiedler eigenvalue; in a build context it means slowest compile step.

### Fluid → Machine: Expression Compilation

```
Fluid: Laplacian(eigenvalue = 2)
          ↓
Machine: SpectralDashboard.recompute()
          → graph.fiedler_value() via power iteration
          → returns λ₂ ≈ 0.34
```

The functor takes a fluid intent and compiles it into machine-executable operations, passing the minimum parameters needed.

### Machine → Fluid: Result Extraction

```
Machine: λ₂ = 0.34, h = 0.21, τ = 3
          ↓
Fluid: ResultInterpreter.assess(λ₂, h, τ)
          → "Agents are reasonably well-connected (λ₂=0.34)
             but agent C is a bottleneck (h=0.21)"
```

The functor takes raw computed values and extracts their significance — what does this number *mean* in context?

### Fluid → Natural: Explanation Extraction

```
Fluid: "Agent C is bottleneck, suggest adding more sessions"
          ↓
Shell: "🔍 Agent connectivity is good overall, but agent C is
         overloaded — it's the bottleneck in your workflow.
         Try opening a second session for agent C to distribute load."
```

The functor takes fluid interpretation and renders it as natural language for the human to read.

---

## Module Positions in the Three-Layer Model

| Module | Layer | Rationale |
|--------|-------|-----------|
| `math_analysis/spectral_dashboard.rs` | Machine core | Eigenvalue computation, power iteration, shift-invert solver — pure Rust math |
| `math_analysis/command_markov.rs` | Machine core | Markov chain transition matrices, stationary distribution via power method |
| `math_analysis/error_hodge.rs` | Machine core | Scoring heuristics for error decomposition — deterministic computation |
| `math_analysis/verification_entropy.rs` | Machine core | Entropy formula computation — O(1), pure math |
| `griot_history/decay.rs` | Machine → Fluid | Decay model is pure computation (Machine), but retelling boosts and pruning require Fluid adaptation |
| `griot_history/persistence.rs` | Fluid | Persistence barcode rendering — adapts machine data to visual representation |
| `griot_history/pattern.rs` | Fluid | Pattern mining and plateau detection — context-sensitive analysis |
| `griot_history/adinkra.rs` | Fluid | Project type detection and alias suggestion — relies on filesystem, adaptive |
| `context_trigger/` | Fluid (hot path) | Trigger dispatch, lifecycle management — event-driven, adaptive |
| `module_system/` | Fluid (hot path) | Module registry, lifecycle FSM (Dormant→Active→Deactivated), memory budgets |
| `forecast/` | Machine → Fluid | Prediction kernels are Machine, but anomaly detection and resource forecasting require Fluid adaptation |
| `ui/` | Fluid ↔ Machine boundary | TUI rendering is Machine-tight (ratatui), but content population is Fluid-adaptive |

---

## Architectural Invariants

1. **No layer skips.** Data flows through all three layers in both directions. The shell never directly calls Machine. The Machine never directly addresses the shell. The Fluid layer is the mandatory intermediary.

2. **Boundaries are explicit.** Each boundary is a well-defined functor with a specific type signature. You can identify the boundary by looking at the module interface — if it takes natural text, it's the Natural→Fluid boundary. If it returns natural text, it's the Fluid→Natural boundary.

3. **The Machine layer is pure.** No I/O, no side effects, no external calls. Given the same inputs, it produces the same outputs. This is what makes it a "fixed point" — stable, predictable, testable.

4. **The Fluid layer is stateless by default.** State accumulates only when needed for adaptation (cached eigenvalues, session history). Stateless operations don't need Fluid persistence — they can go directly Machine→shell (via Fluid's extraction functor).

5. **Durability increases inward.** Natural is ephemeral (scrollback). Fluid is session-persistent. Machine is permanent (binary). The durability gradient means inner layers can depend on inner layers being there; outer layers can be reconstructed.

---

## Why This Corrects the Old Model

The original "shell as fallback" view was:

```
Machine
  ↕
Fluid
  ↕
Shell (fallback)
```

This inverted the architecture. In practice:

- **The shell is not a fallback** — it's the primary interface. Users don't "fall through" from Machine to Shell. They *start* in the shell and *stay* there.
- **Fluid is not optional** — it's the hot path. Without Fluid, there's no adaptation, no context sensitivity, no module lifecycle.
- **Machine is not the top** — it's the foundation. Pure computation is what everything builds on, but it's the innermost, most abstract layer, not the outermost visible one.

The concentric model is:

```
NATURAL (outermost — primary interface)
  FLUID (middle — hot path, adaptation)
    MACHINE (innermost — fixed point, pure computation)
```

This is the correct model. The shell is where everything begins and ends. The machine is the engine room, not the control room.

---

## Related Documents

- [CORRECTED_MODEL.md](./docs/CORRECTED_MODEL.md) — Concise explanation of why the fallback model was wrong
- [ARCHITECTURE.md](./ARCHITECTURE.md) — Module system design, lifecycle, memory budgets, feature gating
- [CONTRIBUTING.md](./CONTRIBUTING.md) — Project contribution guidelines
- [docs/CONTRIBUTING.md](./docs/CONTRIBUTING.md) — Shell-layer durability contract
