# Agents as Applications: open-terminal

## The Terminal IS the Agent Interface

In traditional terminals, you type commands and see text output. In open-terminal, **the terminal is the agent's interface**. The agent authenticates via lattice-crypto, shows real-time telemetry overlays, and suggests commands based on conservation state.

### How It Works

1. **Agent Overlay**: The `agent_overlay` module renders real-time telemetry directly in the terminal. Conservation budget, fleet health, and spectral ranking appear as an overlay that doesn't interfere with the user's workflow.

2. **Conservation Budget Display**: Shows how much computational energy the agent fleet is consuming. A bar chart visualizes the budget as a finite resource — when it's full, the fleet is at capacity. The agent can't overcommit.

3. **Fleet Health**: Each agent in the fleet reports its health score, spectral rank, and task count. The overlay shows which agents are active (●) vs. idle (○), their health as a progress bar, and the fleet's overall spectral ranking.

4. **Spectral Ranking**: The fleet's spectral rank (dominant eigenvalue of the agent interaction matrix) appears in the overlay. A high spectral rank means the fleet is well-coordinated; a low rank suggests fragmentation.

### Architecture

```
┌────────────────────────────────────────────┐
│  open-terminal                             │
│                                            │
│  ┌────────────────────────────────────┐    │
│  │  Agent Overlay (renderer)          │    │
│  │                                    │    │
│  │  ╭─ Conservation Budget ──────╮    │    │
│  │  │ [████████░░░░░░░] 53.2%    │    │    │
│  │  │ Active:   3  Pending:  2   │    │    │
│  │  ╰───────────────────────────╯    │    │
│  │                                    │    │
│  │  ╭─ Fleet Health ─────────────╮    │    │
│  │  │ ● agent-main  [████████░░] │    │    │
│  │  │ ○ agent-worker [██████░░░] │    │    │
│  │  │ Fleet: 82%  Spectral: 0.74 │    │    │
│  │  ╰───────────────────────────╯    │    │
│  └────────────────────────────────────┘    │
│                                            │
│  User's terminal session below             │
└────────────────────────────────────────────┘
```

### Authentication

The agent authenticates to the terminal via lattice-crypto (post-quantum). The terminal provides a lattice-based challenge; the agent responds with a proof of identity. This ensures only authorized agents can display overlays and access terminal state.

### Files

- `src/renderer/agent_overlay.rs` — Agent overlay renderer with budget, fleet health, and spectral ranking display
