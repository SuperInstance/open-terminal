# Agents as Applications: open-terminal

> The terminal doesn't just run commands. It *is* the agent interface.

## The Paradigm Shift

Traditional terminals are passive: you type a command, it executes, you see output. **open-terminal** (wta) breaks that model. The terminal is an active agent that monitors system resources, suggests commands, and enforces conservation laws. It doesn't wait for you to ask — it tells you what's happening and what to do about it.

The agent *is* the terminal. The monitoring doesn't happen *to* the user. The terminal *performs* monitoring as its native mode of existence.

## Conservation Monitoring (γ + H = C)

The core insight: system resources obey a conservation law. **γ (active work) + H (idle/waste) = C (total capacity)**. This isn't a metaphor — it's a mathematical constraint. If γ + H > C, you've overcommitted. The terminal detects this and raises the alarm.

### How It Works

The `ConservationMonitor` takes periodic samples of CPU and memory usage:
- **γ**: Active CPU utilization (normalized to budget)
- **H**: Idle capacity (C - γ, or measured independently)
- **C**: Total budget (number of cores, or configured capacity)

Each sample produces a `ConservationReport` with:
- Current γ, H, and C values
- Whether the system is overcommitted
- The violation magnitude
- Memory pressure indicator

The monitor uses a weighted average (70% current + 30% historical) to smooth out noise while remaining responsive to sudden changes.

### Conservation Alert

When γ + H > C (beyond tolerance):
1. **Diagnostic**: `ps aux --sort=-%cpu | head -10` — find the CPU hogs
2. **Cleanup**: Pause background builds, suggest killing non-essential processes
3. **Escalation**: If persistent for 3+ samples, recommend workload redistribution

## Command Suggestions

The `command_suggest` module uses the conservation state to suggest actions:

| State | Category | Example Commands |
|-------|----------|-----------------|
| Overcommitted | Diagnostic | `ps`, `top`, `free` |
| Overcommitted | Cleanup | `kill`, `pkill`, pause builds |
| Memory pressure | Diagnostic | `free -h`, `ps --sort=-%mem` |
| Underutilized | Productive | `cargo build`, `cargo test` |
| Very idle | Maintenance | `cargo clippy`, lint checks |
| Healthy + busy | Diagnostic | `top -bn1` monitoring |

### Categorical Composition

The `compose_suggestions` function chains suggestions across time series:
- Persistent overcommit → escalating urgency
- Memory trend detection → warn about leaks
- Utilization trends → proactive scheduling

This is categorical-agent composition: the composition of two conservation states yields a suggestion that respects both.

## Spectral Command Ranking

Commands are also ranked spectrally: frequently useful commands (like `git status`, `cargo build`) have higher eigenvalue weight in the command graph. The terminal learns your usage patterns and surfaces the most relevant commands first.

## Lattice-Crypto Authentication

Agent sessions use lattice-based cryptography for authentication. The terminal doesn't trust commands blindly — it verifies the agent's identity through post-quantum cryptographic proofs. This ensures that agent suggestions can't be tampered with by a man-in-the-middle.

## Architecture

```
tools/wta/src/agent/
├── mod.rs                    — Module declaration
├── conservation_monitor.rs   — γ+H=C monitoring (12+ tests)
└── command_suggest.rs        — State-aware command suggestions (12+ tests)
```

The module integrates with the existing wta infrastructure:
- `ConservationReport` feeds into the TUI dashboard
- `SuggestedCommand` appears in the command popup
- Conservation state influences agent session scheduling

## Integration Points

- **open-iterator**: Conservation budget enforcement in editor annotations
- **open-parallel**: Resource monitoring feeds into spectral scheduling
- **open-tui**: Dashboard widgets display conservation state in real-time
- **open-mind**: Conservation patterns enable code analysis induction
- **open-application**: Capability loader discovers terminal capabilities via CAPABILITY.toml

## Future Directions

- GPU conservation monitoring (VRAM budget)
- Network bandwidth as a conservation dimension
- Cross-machine conservation aggregation
- Machine learning for command suggestion personalization
- Integration with container orchestration (Docker, Kubernetes)
