# Contributing: Shell-Layer Durability Contract

> **Audience:** Module authors and contributors to the SuperInstance enhancements.
> This document describes the **durability contract** every module must satisfy before being marked stable.

---

## The Core Principle

The Intelligent Terminal is, first and foremost, a **terminal**. The shell must never go down. A module that crashes, hangs, or delays the shell is worse than no module at all.

The durability contract ensures that no module can bring down the terminal, even when the module itself is broken.

```
  ┌──────────────────────────────────────────────┐
  │              SHELL ALWAYS RUNS                │
  │                                              │
  │  If a module fails:                          │
  │    → The shell keeps running                 │
  │    → The module enters Degraded state        │
  │    → The user gets a notification            │
  │    → No data loss (state flushed to disk)    │
  │                                              │
  │  The shell-layer durability contract         │
  │  guarantees this. No exceptions.             │
  └──────────────────────────────────────────────┘
```

---

## Every Module Needs a Shell Fallback

Every module — every single one — must define what happens when it is unavailable, slow, or broken. This is the **shell fallback**.

### What "Shell Fallback" Means

A shell fallback is not "run the same computation in the shell." It's:

1. **Graceful degradation.** What does the terminal do when this module is broken? The answer is never "crash" or "hang." The answer is always a safe no-op or a useful degraded output.

2. **User notification.** The user must know the module is degraded. A status indicator, a log message, or a UI element. Not a crash dialog.

3. **No data corruption.** If the module fails mid-operation, the shell state must remain consistent. No partial writes, no corrupted config files.

### Example: Spectral Dashboard Fallback

The spectral dashboard computes Fiedler eigenvalues. If it fails (bad graph data, numerical instability):

```
// Fallback behavior — not shown to user as error,
// silently returns None. The UI checks this and shows
// "—" or a grayed-out metric instead of a number.
pub fn fiedler_value(&self) -> Option<f64> {
    if self.graph.num_nodes() < 2 {
        return None;     // graceful: no multi-agent graph, no metric
    }
    // ... computation that might panic ...
    // But the caller wraps in a try block
}
```

At the UI level:

```rust
// The status bar shows the metric only if available.
// If the module is degraded, it shows "—" or the
// last known value with a ⚠ indicator.
fn render_spectral_status(&self) -> String {
    match self.spectral_dashboard.fiedler_value() {
        Some(v) => format!("λ₂={:.2}", v),
        None => "λ₂=—".to_string(),   // graceful degradation
    }
}
```

### Example: Griot History Fallback

The griot history module might fail if the persistence file is corrupt:

```rust
fn load_persistence(path: &Path) -> Result<DecayModel, PersistenceError> {
    let data = std::fs::read_to_string(path)?;
    let model: DecayModel = serde_json::from_str(&data)?;
    Ok(model)
}

// In practice, the module handles this gracefully:
fn load_or_default(path: &Path) -> DecayModel {
    match load_persistence(path) {
        Ok(model) => model,
        Err(e) => {
            log::warn!("Griot history corrupt at {}: {}; starting fresh", path.display(), e);
            DecayModel::new()   // fallback: start fresh
        }
    }
}
```

---

## The Fallback Ships Before the Module Is Marked Stable

This is a hard rule:

> A module's shell fallback must be **shipped, reviewed, and tested** before the module itself can be marked stable.

### Rationale

- **Stable means safe.** A module without a fallback is a single point of failure. If it breaks, the shell breaks.
- **Testing degradation is harder than testing the happy path.** The fallback must be tested too — and that testing happens more naturally when the fallback is considered part of the initial module design, not an afterthought.
- **It forces good design.** A module designed with a fallback from day one has cleaner error handling, better isolation, and fewer implicit assumptions about infrastructure.

### Process

1. **Design the fallback first.** Before writing the main computation path, write the `match` or `if` branches that handle absence, failure, and degradation.

2. **Test the fallback.** At minimum, a test that simulates module failure and verifies the shell continues normally.

3. **Document the fallback.** In the module's `mod.rs` doc comment, describe the failure modes and what happens in each case.

4. **Flag as unstable.** Until the fallback is reviewed, the module must be marked `unstable` in its feature documentation. No exceptions.

```
┌─────────────────────────────────────────────┐
│             Module Lifecycle                 │
│                                             │
│  [Dormant]                                  │
│     │  Feature flagged, no fallback yet     │
│     ▼                                       │
│  [Fallback Shipped]                         │
│     │  Fallback exists, tested, reviewed    │
│     ▼                                       │
│  [Stable]                                   │
│     │  Main computation + fallback both     │
│     │  passing all tests                    │
│     ▼                                       │
│  [Deprecated] → [Evicted]                   │
└─────────────────────────────────────────────┘
```

---

## Module Lifecycle: Registered → Activated → Degraded → Evicted

Each module moves through a lifecycle tracked by the Module Registry. The lifecycle ensures that broken modules never take down the shell.

### Registered (Compiled In)

The module exists in the binary (feature gate enabled) but has never been triggered. No memory allocated, no state initialized.

### Activated (Running)

The module received its first trigger event and successfully initialized. It is actively processing events, computing results, and contributing to the terminal's behavior.

```
┌─ First trigger ───────────────────────────┐
│  ModuleRegistry.activate("module_name")    │
│  → terminal_module.activate(ctx)          │
│  → if Ok: state = Active                  │
│  → if Err: state = Degraded               │
└────────────────────────────────────────────┘
```

### Degraded (Partial Failure)

The module experienced an error during activation, tick, or event handling. In this state:

- The module's `tick()` and event handlers are **skipped** (not called)
- Previously computed state is **preserved** (not freed)
- The module registry logs the error and increments a failure counter
- The UI may show a degradation indicator
- The module can attempt re-activation on the next trigger event
- After `MAX_DEGRADED_COUNT` consecutive failures, the module enters Evicted

```rust
pub fn handle_module_error(&mut self, module_id: &str) {
    if let Some(state) = self.modules.get_mut(module_id) {
        state.degraded_count += 1;
        if state.degraded_count >= MAX_DEGRADED_COUNT {
            state.state = ModuleState::Evicted;
            log::error!("Module {} evicted after {} failures", module_id, state.degraded_count);
        } else {
            state.state = ModuleState::Degraded;
            log::warn!("Module {} degraded (attempt {}/{})",
                module_id, state.degraded_count, MAX_DEGRADED_COUNT);
        }
    }
}
```

### Evicted (Permanently Disabled)

The module exceeded `MAX_DEGRADED_COUNT` failures. In this state:

- The module is **permanently disabled** for the current session
- Its state is **flushed to disk** if it implements `deactivate()`
- Its memory is **freed**
- A notification is sent to the user
- The module can only be re-enabled by restarting the terminal

```
┌─ Eviction Sequence ────────────────────────┐
│  1. Module enters Degraded again           │
│  2. degraded_count >= MAX_DEGRADED_COUNT   │
│  3. Call terminal_module.deactivate()      │
│     → flush state, release resources       │
│  4. Set state = Evicted                    │
│  5. Free module heap allocation            │
│  6. Notify user via agent status bar       │
│  7. ModuleRegistry.remove(module_id)       │
└────────────────────────────────────────────┘
```

---

## The Contract Checklist

Before marking any module as stable, verify:

- [ ] **Fallback exists.** What happens when this module fails? Define it.
- [ ] **Fallback is tested.** A test exists that simulates module failure and verifies shell continuity.
- [ ] **Fallback is shipped.** The fallback code is in the repository, reviewed, and merged.
- [ ] **Degraded state is handled.** The Module Registry transitions the module through the lifecycle correctly.
- [ ] **No shell crash.** A module panic is caught and logged, not propagated to the shell.
- [ ] **No shell hang.** Module operations have timeouts (configurable per module).
- [ ] **No data loss.** Module state is flushed on deactivation.
- [ ] **User notification.** The user knows when a module is degraded or evicted.
- [ ] **Documentation.** The module's `mod.rs` documents failure modes and fallback behavior.

---

## Related Documents

- [THREE_LAYER_ARCHITECTURE.md](../THREE_LAYER_ARCHITECTURE.md) — The architecture this contract protects
- [CORRECTED_MODEL.md](./CORRECTED_MODEL.md) — Architectural model correction
- [ARCHITECTURE.md](../ARCHITECTURE.md) — Module system design, lifecycle implementation
