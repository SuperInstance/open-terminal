# WTA — Intelligent Terminal

## Universal Harness for AI Agents, Reflexes & Mathematical Awareness

WTA is a **universal harness** — it doesn't re-implement anything. It imports and wires.

Every module is an optional connection to a SuperInstance metal library or system:
spectral graphs from `spectral-graph-agent-rs`, Hodge decompositions from `hodge-belief-rs`,
sheaf cohomology from `sheaf-agents-rs`, reflexes from `pincherOS`. All feature-gated.
Zero cost when disabled. Ships with a shell fallback before it's marked stable.

```
cargo build --features all                 # Everything — metal libs, reflexes, entropy, trending
cargo build --features math-tools          # Spectral graph, Hodge, Markov, entropy
cargo build --features pincher             # PincherOS reflex engine
cargo build --features trending            # Clone → analyze → decompose → absorb any GitHub repo
```

> **The terminal doesn't wait for you to ask. It watches the math and speaks when the math says something is wrong. When the math can't decide, the shell waits. When the shell can't decide, you type. The fallback chain is never broken.**

---

## Quick Start

### Build

```bash
cd tools/wta
cargo build --features all
```

The binary is output to `tools/wta/target/debug/wta.exe`.

### Run (ACP TUI mode)

```bash
# Default agent (Copilot)
wta

# With a specific agent
wta --agent "copilot --acp --stdio"

# Claude via ACP adapter
wta --agent "claude-agent-acp --stdio"
```

### tmux-like CLI

```bash
wta list-windows                          # list all WT windows
wta list-tabs                             # list tabs
wta list-panes                            # list panes
wta new-tab -c "pwsh.exe" -n "Build"      # create tab running pwsh
wta split-pane -H -c "pwsh.exe"           # split horizontal
wta capture-pane -t 3 -l 50              # read last 50 lines
```

Short aliases: `lsw`, `lst`, `lsp`, `neww`, `splitw`, `capturep`.

### Protocol Discovery

```bash
wta pipe-id                               # print CLSID
wta test-pipe                             # test WT protocol connection
eval "$(wta set-env)"                     # re-export CLSID (bash/zsh)
```

WTA finds Windows Terminal via `WT_COM_CLSID` — inherited from every conpty child.
Usually zero-config.

## Protocol Connection

WTA discovers Windows Terminal via the `WT_COM_CLSID` environment variable. WT
sets this in its own environment at startup and propagates it to every conpty
shell, so any pane-launched process — including wta and wtcli — inherits it.

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `WT_COM_CLSID` | Yes* | Stringified GUID of WT's `TerminalProtocolComServer` COM class |
| `WTA_DEBUG_LOG` | No | Set to `0` to disable `wta-pipe-debug.log` |

\* Set automatically by WT when it spawns a conpty child. If you launch `wta` from outside WT, run `eval "$(wta set-env)"` to copy the value over (only useful when you've previously captured it from a WT shell).

## Global CLI Options

| Flag | Description |
|------|-------------|
| `--json` | Output raw JSON instead of human-readable tables |
| `--agent <CMD>` | Agent CLI command for ACP mode (default: `copilot --acp --stdio`) |

## TUI Controls

| Key | Action |
|-----|--------|
| Type + Enter | Send prompt to agent |
| Ctrl+C | Cancel streaming / quit |
| PageUp / PageDown | Scroll chat |
| F12 | Toggle debug panel (pipe traffic viewer) |
| Shift+PageUp/Down | Scroll debug panel |
| Y / N | Quick allow/reject on permission dialog |
| Up / Down / Enter | Navigate permission options |

## Debug Panel

Press **F12** to open a side panel showing all JSON-RPC messages between WTA and Windows Terminal in real time.

```
[3456.1] >>> {"type":"request","id":"3","method":"list_windows","params":{}}
[3456.1] <<< {"type":"response","id":"3","result":{"windows":[...]},"error":null}
```

- Green `>>>` = request sent to WT
- Cyan `<<<` = response from WT
- Shift+PageUp/Down to scroll

## Debug Logs

WTA writes structured logs under the package log dir, in a per-version
subfolder: `…\LocalCache\Local\IntelligentTerminal\logs\<pkgver>\` when
packaged (or bare `%LOCALAPPDATA%\IntelligentTerminal\logs\` unpackaged):

| File | Contents |
|------|----------|
| `wta-main.log` | Main TUI runtime: lifecycle, agent events, protocol calls |
| `terminal-agent-pane.log` | Agent-pane chrome (C++ TerminalApp side) |
| `wta-ensure-host.log` | Background host startup / COM connection |
| `wta-acp-debug.log` | ACP protocol debug trace |
| `wta-delegate.log` | `?<prompt>` delegation flow |
| `wta-attach.log` | Agent pane TUI in attach mode |

Set `WTA_LOG=debug` for verbose output (default: `info`). The F12 debug panel
in the TUI shows protocol traffic live without tailing log files.

## Project Structure

```
tools/wta/src/
+-- main.rs                    Entry point, CLI subcommands, protocol discovery
+-- app.rs                     TUI state machine, event loop, debug panel state
+-- event.rs                   Crossterm event reader
+-- theme.rs                   Color constants
+-- protocol/
|   +-- acp/client.rs          ACP client -- spawns agent, handles requests
+-- shell/
|   +-- shell_manager.rs       Terminal abstraction (local subprocess or WT pane)
|   +-- wt_channel/
|       +-- mod.rs             WtChannel trait definition
|       +-- cli_channel.rs     wtcli subprocess (CoCreateInstance via wtcli.exe) — all methods
+-- ui/
    +-- layout.rs              Main layout (+ debug panel split)
    +-- chat.rs                Message rendering
    +-- input.rs               Input box with cursor
    +-- status_bar.rs          Connection status, pane identity, debug hint
    +-- permission.rs          Permission modal dialog
    +-- debug_panel.rs         Protocol traffic viewer (F12)
```

## Development

### Prerequisites

- Rust toolchain (edition 2021)
- Windows Terminal with protocol server enabled (for WT integration)
- An ACP-compatible agent CLI (Copilot, Claude ACP adapter, etc.)

### Build and run

```bash
cd wta
cargo build

# Option 1: Auto-discover pipe (run inside Windows Terminal)
target/debug/wta.exe

# Option 2: Set env vars for the session
eval "$(target/debug/wta.exe set-env)"
target/debug/wta.exe
```

### Development workflow

1. Open Windows Terminal
2. Run `wta pipe-id` to verify `WT_COM_CLSID` is set
3. Run `wta` to start the TUI
4. Press F12 to open the debug panel and see all protocol traffic
5. Interact with the agent -- watch requests/responses flow in real time
6. Use `wta list-panes`, `wta capture-pane` etc. in another pane for debugging

### Adding a new WT protocol method

1. Declare the method in `src/cascadia/TerminalProtocol/TerminalProtocol.idl`
2. Implement it on `TerminalProtocolComServer` (`src/cascadia/WindowsTerminal/TerminalProtocolComServer.cpp`)
3. Add a `wtcli` subcommand in `src/tools/wtcli/main.cpp` that calls the new method
4. Add a `CliChannel::request` arm in `tools/wta/src/shell/wt_channel/cli_channel.rs` mapping a method name to the new `wtcli` subcommand
5. Rebuild WT, wtcli, and wta

## Harness Architecture

The terminal is organized as three concentric layers, each with a dual-aspect functor at its boundary:

```
NATURAL (outermost) ── Shell interface, interpreted by humans & LLMs
  ↑ Intent compilation (you say "find the bottleneck") 
  ↓ Explanation extraction (terminal tells you what it found)
───
FLUID (hot path) ── A proper language in the transformation graph
  ↑ Expression compilation (fluid picks the algorithm)
  ↓ Result extraction (eigenvalue → fluid decides what it means)
───
MACHINE (cold path) ── Compiled Rust. Bit-identical. Slow-to-change.
```

The shell is not a fallback — it is the **outermost ring interface**. Every module has a shell expression.

### Connected Systems

Every row is an **optional feature gate**. Zero cost when disabled.

| Feature | Crate | What It Does |
|---------|-------|-------------|
| `math-tools` | `spectral-graph-agent-rs` | QR eigenvalue decomposition → Fiedler, Cheeger, mixing time |
| `math-tools` | `hodge-belief-rs` | Error classification: evidence / coherence / prior |
| `math-tools` | `sheaf-agents-rs` | H⁰/H¹ disagreement on agent communication graphs |
| `math-tools` | `ergodic-transport-rs` | Markov chain prediction → command forecast |
| `math-tools` | `conservation-spectral-topology-rs` | Cheeger inequality for spectral gap bounds |
| `math-tools` | `evolving-sheaf-rs` | RG flow on skill plateaus |
| `math-tools` | `conservation-sheaf-flow-rs` | Static sheaf theorem verification |
| `griot-history` | `west-african-math-rs` | Decay, pattern mining, adinkra compression, persistence |
| `pincher` | `pincherOS` | Every command teaches a reflex. 5-command workflows auto-compile. Errors auto-learn handlers. |
| `trending` | (standalone) | Clone any GitHub repo → analyze → decompose → absorb as a terminal module |
| `griot-history` | `renormalization-learning-rs` | Universality class detection + skill plateau prediction |
| `math-tools` | `free-probability-rs` | Marcenko-Pastur bounds for initialization theory |
| — | `terminal-spectral-harness` | Standalone crate: wraps spectral-graph-agent-rs for UI |
| — | `terminal-entropy-harness` | Standalone crate: VerificationEntropy as independent library |

### The Loop

Every command compiles a better reflex (PincherOS). Every error improves the decomposition (Hodge). Every skill plateau accelerates the next RG step (renormalization). The bottleneck scales arbitrarily because the bottleneck is you.

```
You type → Markov predicts → PincherOS compiles → Reflex runs at 50ms
You break → Hodge decomposes → you learn → next time the reflex handles it
You plateau → Renormalization detects → PincherOS teaches → gap closes
```

### Project Structure

```
tools/wta/src/
├── main.rs                    Entry, CLI subcommands, protocol discovery
├── app.rs                     TUI state machine, event loop
├── event.rs                   Crossterm event reader
├── theme.rs                   Color constants
├── reflex_bridge.rs           PincherOS: teach reflexes, detect workflows, learn errors
├── trending_harness.rs        Clone → analyze → decompose → absorb any repo
├── math_analysis/             Feature-gated: Markov, Hodge, entropy, spectral
├── griot_history/             Feature-gated: decay, pattern, persistence, skill detection
├── context_trigger/           Feature-gated: 7 auto-activation rules
├── module_system/             Feature-gated: TerminalModule trait, LRU memory budget
├── forecast/                  Feature-gated: command prediction via ergodic theory
├── protocol/
│   └── acp/client.rs          ACP client — spawns agent, routes requests
├── shell/
│   ├── shell_manager.rs       Terminal abstraction: local subprocess or WT pane
│   └── wt_channel/            wtcli subprocess — all WT methods via COM
└── ui/
    ├── entropy_bar.rs         Always-visible verification entropy gauge
    ├── agent_disagreement.rs  H⁰/H¹ disagreement visualization
    ├── layout.rs              Main layout + debug panel split
    ├── chat.rs                Message rendering
    ├── input.rs               Input box with cursor
    ├── status_bar.rs          Connection status, pane identity
    ├── permission.rs          Permission modal
    └── debug_panel.rs         Protocol traffic viewer (F12)
```

### Architecture Invariants

1. **Every module is feature-gated.** Zero cost when disabled.
2. **Every module has a shell fallback.** The fallback ships before the module is marked stable.
3. **The terminal re-implements nothing.** It imports and wires.
4. **The three layers are concentric.** Shell is the outermost ring, not a fallback.
5. **Every boundary is a dual-aspect functor.** Compilation runs down, extraction runs up.

### Graceful Degradation

If the WT protocol is unavailable, WTA falls back to local-only mode.
If the math runtime fails, the shell fallback executes.
If the reflex engine can't compile, the Markov chain predicts.
If the chain can't predict, the shell prompt waits.
If the shell can't decide, you type.

The fallback chain is never broken. This is what makes the terminal a tool rather than a dependency.

### Architecture Notes

- **ShellManager** owns local terminals and the active `WtChannel`
- **CliChannel** shells out to `wtcli.exe` per call; `wtcli` does `CoCreateInstance` to reach WT's COM server
- **Protocol discovery**: `WT_COM_CLSID` env var, inherited from WT-spawned conpty
- **Pane identity**: discovered at startup via PID matching
- **Graceful degradation**: local-only mode when WT protocol unavailable
