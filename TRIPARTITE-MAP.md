# Tripartite Map — Intelligent Terminal → SuperInstance Synchronizer

> Which parts should be HARDCODED, MODELED, or CACHED, using the tripartite synchronizer logic.

## Principles

| Layer | Meaning | Latency | Mutability | Examples |
|-------|---------|---------|------------|----------|
| **HARDCODE** | Deterministic, latency-critical, compiled-in | <1ms | Never (code change) | Rendering, state machines, trigger predicates |
| **MODEL** | Creative, contextual, requires judgment | 100ms-10s | Per-request | Suggestions, diagnoses, predictions, explanations |
| **CACHED** | Persistent state, reflexes, historical patterns | <10ms | Eviction-based | Command history, transition matrices, resource stats |

---

## HARDCODE — Deterministic, Latency-Critical

### Terminal Rendering (`ui/`)
All ratatui rendering is frame-clocked and must complete in <16ms. No model calls in the render path.

- `ui/layout.rs` — Adaptive layout engine
- `ui/input.rs` — Input handling and cursor tracking
- `ui/chat.rs` — Message rendering
- `ui/card.rs` — Card component system
- `ui/shimmer.rs` — Animation frame calculation

**Why HARDCODE:** Frame budget is fixed. Any model call in the render loop causes dropped frames and visible jank.

### Trigger Predicates (`context_trigger/triggers.rs`)
Pure `fn(&TerminalEvent) -> bool` functions. Stateless, allocation-free. Must evaluate in <1µs per event.

- `math_tools_trigger`
- `error_hodge_trigger`
- `verification_entropy_trigger`
- `spectral_dashboard_trigger`

**Why HARDCODE:** Triggers run on *every* terminal event. They are the gate that prevents unnecessary model/analysis work. They must be faster than the events they gate.

### Lifecycle State Machine (`context_trigger/dormant.rs`)
Four-state FSM with strict transition rules. Single-threaded, no IO.

**Why HARDCODE:** State transitions are correctness-critical. Invalid transitions would corrupt module lifecycle. Must be deterministic.

### Error Hodge Decomposition (`math_analysis/error_hodge.rs`)
The scoring algorithm is deterministic: given an error, it computes evidence/coherence/prior-mismatch scores via fixed rules. No randomness, no model calls.

**Why HARDCODE:** The decomposition itself is a mathematical operation. The *interpretation* of the decomposition is MODEL (see below).

### Agent Profiles (`agent_registry.rs`)
Static catalog of agent capabilities, CLI flags, auth flows. Compiled-in metadata.

**Why HARDCODE:** Agent CLI interfaces change slowly (version updates, not per-request). This is configuration, not intelligence.

---

## MODEL — Creative, Contextual, Requires Judgment

### Agent Suggestions (`forecast/predictor.rs` → ghost text)
The top-K prediction ranking is partially MODEL: while transition probabilities are computed from CACHED data, the *selection* of which predictions to show (and how to phrase them) benefits from contextual understanding.

- Next-command prediction display
- Ghost text formatting
- "You might want to..." suggestions

**Why MODEL:** The raw probabilities are deterministic, but choosing the right suggestion *for this user in this context* requires judgment. "After `cargo build`, run `cargo test`" is obvious; "After `git stash`, consider checking your branch" requires understanding intent.

### Error Diagnosis (`error_hodge` decomposition → interpretation)
The Hodge decomposition produces scores (HARDCODE), but the *diagnosis* — "this looks like a version mismatch" or "you're missing a dependency" — requires MODEL reasoning.

- Error explanation generation
- Fix suggestion synthesis
- Prior mismatch resolution

**Why MODEL:** Evidence/coherence/prior_mismatch are numbers. Turning those numbers into "here's what went wrong and how to fix it" requires world knowledge.

### Workflow Anomaly Interpretation (`forecast/anomaly.rs` → explanation)
KL divergence and Wasserstein distance are numbers (HARDCODE computation). "Your workflow shifted because you started a new feature branch" is MODEL.

- Workflow shift explanation
- Task transition detection
- Context change recommendations

**Why MODEL:** The divergence scores tell you *that* something changed. Understanding *what* changed requires reasoning about intent.

### Agent Disagreement Resolution (`ui/agent_disagreement.rs`)
H⁰/H¹ computation is HARDCODE (graph Laplacian eigenvalues). But resolving disagreements — "Claude suggests refactoring, Copilot suggests a quick fix, here's the tradeoff" — is MODEL.

- Disagreement summarization
- Resolution recommendation
- Tradeoff analysis

**Why MODEL:** The sheaf cohomology tells you *structure* of disagreement. Choosing between alternatives requires domain judgment.

### Recommendation Synthesis (`coordinator.rs` → recommendation cards)
Merging multiple agent outputs into a coherent recommendation set.

**Why MODEL:** Synthesis of conflicting or complementary suggestions into a single actionable recommendation requires contextual reasoning.

---

## CACHED — Persistent State, Reflexes, Historical Patterns

### Command Transition Matrix (`forecast/transition_matrix.rs`)
The Markov chain state persists across sessions. Loaded at startup, updated incrementally, serialized on shutdown. 100-state matrix with Laplace smoothing.

- Raw transition counts
- Row-stochastic probability matrix
- State index mappings

**Why CACHED:** This *is* pincherOS-style reflexes. "After `git add`, 87% of the time you run `git commit`" is learned behavior cached from history. No model call needed to retrieve — it's a matrix lookup.

### Command Markov Chain (`math_analysis/command_markov.rs`)
512-state chain with stationary distribution and mixing time estimates. Persists across sessions.

- Transition count matrix
- Stationary distribution (cached)
- Mixing time estimate

**Why CACHED:** Long-term behavioral pattern. The stationary distribution tells you "this user spends 30% of time in git commands" — a reflex, not a judgment.

### Agent Session History (`agent_sessions.rs`)
Historical record of all agent sessions: start time, CLI source, session keys, resume capability.

- Session metadata
- Resume keys
- CliSource classifications

**Why CACHED:** Session history is reference data. "You had a Claude session yesterday about fixing the auth bug" is a fact, not a prediction.

### Resource Usage Statistics (`forecast/resource_predictor.rs`)
EMA of memory/CPU/duration per command. Persists across sessions.

- Per-command resource EMAs
- Variance estimates
- Min/max observations

**Why CACHED:** "cargo build typically needs 1.8GB RAM" is a learned reflex. Cached from observation, retrieved by lookup.

### Verification Entropy State (`math_analysis/verification_entropy.rs`)
Running count of edits since last test. Session-scoped but with cumulative totals.

- Lines edited since last test
- Total lifetime edits/tests
- Entropy level history

**Why CACHED:** The entropy calculation is deterministic (HARDCODE formula), but the *state* (how many edits since last test) is cached runtime data.

### Module State (`module_system/memory_budget.rs`)
Serialized module state for LRU eviction. Persisted to disk when modules are deactivated.

- Deactivated module state blobs
- Memory usage tracking
- Trigger ordering

**Why CACHED:** Module state is exactly pincherOS-style: serialized snapshots that can be rehydrated. The serialization format is HARDCODE, the state content is CACHED.

---

## HYBRID — Split Across Layers

### Error Analysis Pipeline
```
Error occurs
    → HARDCODE: Hodge decomposition (evidence/coherence/prior_mismatch scores)
    → CACHED: Historical error patterns for this command
    → MODEL: Diagnosis and fix suggestion
```

### Command Forecasting Pipeline
```
Command entered
    → CACHED: Transition matrix lookup (P(next | current))
    → HARDCODE: Top-K selection and ghost text formatting
    → MODEL: Contextual re-ranking based on current task intent
```

### Agent Collaboration Pipeline
```
Multiple agents active
    → CACHED: Historical collaboration patterns
    → HARDCODE: Spectral analysis (Fiedler value, Cheeger constant)
    → HARDCODE: Sheaf cohomology (H⁰, H¹)
    → MODEL: Disagreement interpretation and resolution
```

### Module Activation Pipeline
```
Terminal event
    → HARDCODE: Trigger predicate evaluation (<1µs)
    → HARDCODE: State machine transition
    → CACHED: Deserialize module state from disk
    → MODEL: (if needed) Contextual module configuration
```

---

## Integration Priority

| Priority | Component | Target Layer | Reason |
|----------|-----------|-------------|--------|
| P0 | Terminal rendering | HARDCODE | Latency critical, frame budget |
| P1 | Trigger predicates | HARDCODE | Event loop gate, must be fast |
| P1 | Transition matrix | CACHED | Foundation for all forecasting |
| P2 | Error Hodge decomposition | HARDCODE | Deterministic math |
| P2 | Agent profiles | HARDCODE | Static metadata |
| P3 | Prediction display | MODEL | Contextual suggestion |
| P3 | Error diagnosis | MODEL | Requires world knowledge |
| P3 | Session history | CACHED | Reference data |
| P4 | Spectral analysis | HARDCODE | Graph math |
| P4 | Disagreement resolution | MODEL | Requires judgment |
| P4 | Resource stats | CACHED | Historical observation |
