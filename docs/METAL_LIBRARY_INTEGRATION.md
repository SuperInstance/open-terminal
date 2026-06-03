# Metal Library Integration

The **Intelligent Terminal** is a **harness** for the SuperInstance metal library
fleet. Every nontrivial math, geometry, or topology computation lives in a
separate Rust crate that this terminal depends on — optionally — via the
`metal-libs` feature gate.

## Philosophy

- The terminal **does not** re-implement spectral graph theory, sheaf
  cohomology, Hodge belief propagation, ergodic transport, or any other
  computational geometry. Those are owned by their respective libraries.
- The terminal **provides** the TUI, the ACP/MCP tool server, the module
  system, the context triggers, and the user-facing UX. It is a **consumer**
  and **test bed**.
- Every math module in the terminal should be replaceable with a direct call
  to a metal library function, keeping the terminal focused on orchestration
  and presentation rather than low-level numerics.

## Library Fleet

| Crate | Purpose |
|---|---|
| `spectral-graph-agent` | Spectral clustering, Laplacian eigendecomposition |
| `conservation-spectral-topology` | Conservation laws via spectral topology |
| `sheaf-agents` | Sheaf-theoretic agent coordination |
| `hodge-belief` | Hodge decomposition for belief propagation |
| `ergodic-transport` | Ergodic trajectory optimization |
| `evolving-sheaf` | Time-varying sheaf structures |

## Feature Gate

Enable all metal libraries with a single cargo flag:

```sh
cargo build -p wta --features metal-libs
```

In `Cargo.toml` the `metal-libs` feature is defined as:

```toml
metal-libs = [
    "spectral-graph-agent",
    "conservation-spectral-topology",
    "sheaf-agents",
    "hodge-belief",
    "ergodic-transport",
    "evolving-sheaf",
]
```

Each library remains an **optional** dependency and can be pulled in
individually if only a subset is needed:

```sh
cargo build -p wta --features spectral-graph-agent
```

## Using the Terminal as a Harness

Because the terminal is a full TUI + MCP tool server, it can serve as an
**interactive test harness** for metal library invariants:

1. **Integration tests** — write Rust `#[cfg(test)]` modules under
   `tools/wta/tests/` that depend on one or more metal libraries and exercise
   round-trip invariants (e.g., `decompose → reconstruct ≈ identity`).
2. **TUI-driven validation** — expose a debug panel that lets a developer
   pick a metal library function, feed it sample data from the terminal's
   ACP context, and inspect the result directly in the terminal UI.
3. **Property-based regression** — use `proptest` or `quickcheck` to generate
   random inputs and verify that the metal library's outputs satisfy the
   mathematical properties documented in its own crate.

### Example Integration Test Skeleton

```rust
// tools/wta/tests/metal_invariants.rs
#![cfg(feature = "metal-libs")]

#[test]
fn spectral_graph_roundtrip() {
    use spectral_graph_agent::SpectralGraph;
    // Build a small graph, decompose, reconstruct, assert.
}
```

## Migration Guide

When porting terminal-internal math to a metal library:

1. Move the algorithm into the appropriate `-rs` crate.
2. Re-export it from the crate's public API.
3. Add the crate as an optional dependency here.
4. Replace the inline implementation with a call to the crate.
5. Delete the old terminal-internal code.

This keeps the terminal lean and the metal libraries independently testable,
versioned, and reusable across projects.
