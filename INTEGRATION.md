# Integration Guide: Ternary Intelligence in Open Terminal

> How the ternary {-1, 0, +1} intelligence system plugs into the Intelligent Terminal's three-layer architecture.

## Overview

The terminal integrates ternary decision logic at the **FLUID layer** — the adaptive, context-sensitive middle ring that sits between the shell interface (NATURAL) and the rendering core (MACHINE). Three ternary modules provide command prediction, pattern analysis, and conservation-law monitoring.

## Ternary Crates & Modules

| Module | File | Role |
|--------|------|------|
| `CommandPredictor` | `tools/wta/src/ternary_integration.rs` | Maps command history to ternary outcomes and predicts next-command suggestions |
| `PatternAnalyzer` | `tools/wta/src/ternary_integration.rs` | Analyzes command transition patterns using ternary strategy lookup tables |
| `ConservationMonitor` | `tools/wta/src/ternary_integration.rs` | Verifies prediction quality by checking that avoidance ratios stay constant across scales (std < 0.01) |

## Integration Points

### 1. Command Prediction — `forecast/predictor.rs`

The `CommandPredictor` feeds into the existing forecast subsystem. When the terminal needs to suggest a next command:

```rust
// In forecast/predictor.rs — ghost text suggestions
use crate::ternary_integration::{CommandPredictor, Trit};

let predictor = CommandPredictor::new(100);
predictor.record("cargo build".into(), Trit::Choose);
predictor.record("cargo clean".into(), Trit::Avoid);

// Returns Choose/Avoid/Unknown for a candidate command
let recommendation = predictor.predict("cargo test");
```

**Where it connects:** The predictor's output is consumed by the ghost text rendering pipeline in `ui/` — HARDCODE layer displays the suggestion, MODEL layer selects phrasing.

### 2. Pattern Analysis — `context_trigger/triggers.rs`

Ternary patterns feed into the trigger predicate system:

```rust
// In context_trigger/triggers.rs — trigger evaluation
// Triggers are pure fn(&TerminalEvent) -> bool — must be <1µs
// PatternAnalyzer provides pre-computed ternary state for trigger predicates

let analyzer = PatternAnalyzer::new(50);
let strategy = analyzer.classify_recent(); // Returns ternary strategy species
// Strategy feeds into math_tools_trigger, error_hodge_trigger, etc.
```

**Where it connects:** Trigger predicates gate all model/analysis work. The pattern analyzer pre-computes ternary classifications so triggers can make fast decisions without model calls.

### 3. Conservation Monitoring — `forecast/anomaly.rs`

Conservation laws verify prediction quality at runtime:

```rust
// In forecast/anomaly.rs — anomaly detection
use crate::ternary_integration::ConservationMonitor;

let monitor = ConservationMonitor::new(0.02); // 2% threshold
monitor.check_conservation(&predictor.history());

// If conservation is violated, the anomaly detector flags it
// KL divergence and Wasserstein distance become suspect
```

**Where it connects:** Conservation violations feed into the workflow anomaly system. When the avoidance ratio drifts, it signals a genuine workflow shift (MODEL layer interprets why).

## Architecture Placement

```
┌──────────────────────────────────────────────┐
│  NATURAL — Shell Interface                   │
│  ┌────────────────────────────────────────┐  │
│  │  FLUID — Ternary modules live here     │  │
│  │  ┌──────────────────────────────────┐  │  │
│  │  │ CommandPredictor (ghost text)    │  │  │
│  │  │ PatternAnalyzer  (triggers)     │  │  │
│  │  │ ConservationMonitor (anomaly)    │  │  │
│  │  └──────────────────────────────────┘  │  │
│  │  ┌──────────────────────────────────┐  │  │
│  │  │ MACHINE — Rendering, state machines│ │
│  │  └──────────────────────────────────┘  │  │
│  └────────────────────────────────────────┘  │
└──────────────────────────────────────────────┘
```

All three modules are **session-persistent** (FLUID layer durability) — state lives for the session and serializes to disk.

## Committed Files

- `484f8ed` — `tools/wta/src/ternary_integration.rs` — full implementation (392 lines)
- `329c5b6` — `TRIPARTITE-MAP.md` — HARDCODE/MODEL/CACHE classification
- `dd1ddbf` — `THREE_LAYER_ARCHITECTURE.md` — concentric architecture document

## Adding New Ternary Modules

1. Implement in `tools/wta/src/ternary_integration.rs`
2. Add HARDCODE/MODEL/CACHED classification in `TRIPARTITE-MAP.md`
3. Connect to the appropriate layer boundary (natural↔fluid or fluid↔machine)
4. Ensure conservation laws hold — test with `ConservationMonitor`
