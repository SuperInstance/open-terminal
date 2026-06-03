<p align="center">
    <picture>
      <img src="./images/intelligent-terminal-logo.png" alt="Intelligent Terminal logo" width="128">
    </picture>
</p>

<h1 align="center">Intelligent Terminal <sup style="color:#58a6ff">+ SuperInstance</sup></h1>

<p align="center"><strong>The first terminal that doesn't wait for you to ask.</strong></p>

<p align="center">
  Your terminal already knows your workflow's mathematics.<br>
  We wired up the gauges.
</p>

---

> **This is a fork of [Microsoft's Intelligent Terminal](https://github.com/microsoft/intelligent-terminal)** — an experimental Windows Terminal with native ACP agent integration. We preserve every line of the original. Then we add mathematical awareness. Zero cost when you don't need it. Profound when you do.

---

<details>
  <summary><strong>Table of Contents</strong></summary>

- [What is Intelligent Terminal?](#what-is-intelligent-terminal)
- [SuperInstance Enhancements](#superinstance-enhancements)
  - [The Idea](#the-idea)
  - [Math-Aware Command Analysis](#math-aware-command-analysis-math-tools)
  - [Griot Command History](#griot-command-history-griot-history)
  - [Zero-Cost Promise](#zero-cost-promise)
  - [Architecture](#architecture)
  - [Enabling Features](#enabling-features)
  - [Coming Soon](#coming-soon)
- [Installing and running Intelligent Terminal](#installing-and-running-intelligent-terminal)
  - [Microsoft Store](#microsoft-store-recommended)
  - [WinGet](#winget)
  - [Downloads](#downloads)
- [Get Started](#get-started)
- [Keyboard Shortcuts](#keyboard-shortcuts)
- [Configuration](#configuration)
- [Features](#features)
  - [Agent Status Bar](#agent-status-bar)
  - [Agent Pane](#agent-pane)
  - [Agent Management](#agent-management)
  - [Error Detection](#error-detection)
  - [Command Palette](#command-palette)
- [Data & Privacy](#data--privacy)
- [Building the Code](#building-the-code)
- [FAQ](./doc/faq.md)
- [Feedback](#feedback)
- [Contributing](#contributing)
- [Code of Conduct](#code-of-conduct)
- [Security](#security)
- [Trademarks](#trademarks)

</details>

<br />

## What is Intelligent Terminal?

Intelligent Terminal is an experimental fork of [Windows Terminal](https://github.com/microsoft/terminal) with native agent integration.

Intelligent Terminal works with any [Agent Client Protocol (ACP)-compatible](https://agentclientprotocol.com/get-started/agents) agent CLI. All you need is to install your preferred agent CLI on your PC. If you don't have a preferred agent, we'll get you setup with [GitHub Copilot CLI](https://github.com/features/copilot/cli/).

Intelligent Terminal takes all the features you love in Windows Terminal such as: tabs, profiles, themes, settings, shells, and keyboard shortcuts, which all work the way you expect.

Read the [announcement blog post](https://devblogs.microsoft.com/commandline/announcing-intelligent-terminal-version-0-1/) for more details.

---

## SuperInstance Enhancements

### The Idea

Intelligent Terminal is already a strong foundation: ACP protocol integration, multi-agent detection, per-tab autofix, session management, and a ratatui-based TUI. But it's a *transport layer* — it moves prompts to agents and renders responses.

The leap from "terminal with an agent pane" to "terminal that *thinks*" requires plugging in mathematical tooling at exactly the right extension points. That's what SuperInstance does.

**Intelligent Terminal + mathematical awareness = the terminal that sees your workflow.**

### Math-Aware Command Analysis (`math-tools`)

Feature-gated under `math-tools`. Four subsystems, each a pure-local computation:

#### Verification Entropy Tracker

Tracks the edit-to-test ratio per session. Verification entropy is *conserved*: every line you edit without running tests accumulates entropy that manifests as latent bugs. This isn't a heuristic — it's a thermodynamic law of software development.

```
┌──────────────────────────────────────────────────┐
│ 📝 Verification │ ▓▓▓░░░░░░░ 350 lines edited   │
│                 │   without testing              │
└──────────────────────────────────────────────────┘
```

Color coding: **Green** (healthy) → **Yellow** (accumulating) → **Orange** (run your tests soon) → **Red** (bugs are coming, conservation of entropy is a law not a suggestion).

#### Hodge Error Decomposition

When a command fails, decomposes the error into three orthogonal components:

- **Evidence** — what actually happened (raw signal)
- **Coherence** — does the error message make internal sense
- **Prior mismatch** — expectation vs reality (your mental model vs what happened)

```
Error: ModuleNotFoundError: No module named 'numpy'

  Prior mismatch: 70% — venv was built with Python 3.12, you're on 3.11
  Evidence:       20% — numpy is genuinely not in this environment
  Incoherence:    10% — the error is clear but the cause isn't stated
```

#### Ergodic Command Markov Chain

Builds a Markov chain from your command history. Your past behavior is a statistically valid predictor of your next command — ergodic theory says time averages converge to ensemble averages. The module predicts resource footprints *before* execution and flags anomalies in real time.

#### Spectral Agent Dashboard

When you're using multiple agents (Copilot, Claude, Codex, Gemini), builds a collaboration graph and computes spectral metrics:

- **Fiedler value** (algebraic connectivity) — how well-connected is your agent network
- **Cheeger constant** — identifies bottleneck agents
- **Mixing time** — how fast information propagates between agents

### Griot Command History (`griot-history`)

Feature-gated under `griot-history`. Inspired by West African knowledge systems, not Silicon Valley metaphors.

#### Decaying Persistence

Each command has a "retelling strength" that decays exponentially. A command from 5 minutes ago is 10× more relevant than one from 5 days ago. But — re-running the same command *strengthens* all prior instances. Frequently-told stories persist longer in memory. That's how griots work.

```
█████░░░░░█████████░░░████░░░░░░░████████
^Rust  ^Node          ^Git        ^Docker
```

#### Adinkra Compression

Detects your project type (Rust, Node, Python, Go…) and suggests context-aware aliases. In a Rust project, `cb` means `cargo build`. In a Node project, `cb` means something else entirely. The terminal adapts to your project, not the other way around.

#### Pattern Mining

Detects repetitive multi-command sequences (e.g., `cargo fmt && cargo clippy && cargo test`) and offers compressed aliases. Detects learning plateaus — when you've been doing the same workflow for 90 days without advancing, it suggests next steps.

### Zero-Cost Promise

All enhancements are **feature-gated**. Disable any feature and the code doesn't exist — not hidden, not dormant, **compiled out**. Zero overhead, zero config.

```toml
[features]
default = []
math-tools = ["nalgebra", "serde"]
griot-history = ["serde"]
all = ["math-tools", "griot-history"]
```

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  Intelligent Terminal                     │
│                                                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐  │
│  │  Agent    │  │  Agent   │  │   Agent Management   │  │
│  │  Pane     │  │  Status  │  │                      │  │
│  │  (ACP)    │  │  Bar     │  │                      │  │
│  └────┬─────┘  └────┬─────┘  └──────────────────────┘  │
│       │              │                                   │
│       ▼              ▼                                   │
│  ┌─────────────────────────────────────────────────┐    │
│  │              Module Registry                     │    │
│  │   (lazy-loaded, feature-gated, event-driven)     │    │
│  │                                                  │    │
│  │  ┌─────────────────┐  ┌──────────────────────┐  │    │
│  │  │  math-tools      │  │  griot-history        │  │    │
│  │  │                  │  │                       │  │    │
│  │  │  • Verification  │  │  • Decay model        │  │    │
│  │  │    Entropy       │  │  • Pattern mining     │  │    │
│  │  │  • Hodge Error   │  │  • Adinkra aliases    │  │    │
│  │  │    Decomposition │  │  • Persistence        │  │    │
│  │  │  • Command       │  │    barcode            │  │    │
│  │  │    Markov Chain  │  │                       │  │    │
│  │  │  • Spectral      │  └──────────────────────┘  │    │
│  │  │    Dashboard     │                             │    │
│  │  └─────────────────┘  ┌──────────────────────┐  │    │
│  │                       │  Coming Soon          │  │    │
│  │                       │  • Context triggers   │  │    │
│  │                       │  • Module registry    │  │    │
│  │                       │  • Spectral dashboard │  │    │
│  │                       │    (live TUI)         │  │    │
│  │                       └──────────────────────┘  │    │
│  └─────────────────────────────────────────────────┘    │
│       │                                                  │
│       ▼                                                  │
│  ┌─────────────────────────────────────────────────┐    │
│  │              Event Bus (AppEvent)                 │    │
│  │  Tick → module.tick() | Key → module.on_input()  │    │
│  │  VT osc:133 → module.on_command_done()           │    │
│  └─────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```


### Three-Layer Architecture

The module system sits inside a **concentric three-layer architecture** that replaces the old "shell-as-fallback" model with a corrected view:

```
NATURAL (outermost ring)
  │  Shell interface — interpreted by humans & LLMs
  │  Intent boundary: natural → fluid translation
FLUID (middle ring — the hot path)
  │  Proper language in the transformation graph
  │  Adaptive, compiled-but-parameterized, context-sensitive
  │  Semantic boundary: fluid → natural extraction
  │  Compilation boundary: fluid → machine  
MACHINE (innermost — the cold path / fixed point)
  │  Compiled Rust. Feature-gated. Zero-cost when disabled.
  │  Bit-identical. Slow-to-change.
```

**The shell is NOT the fallback — it's the outermost ring interface.** In the old model, shell was treated as a lower tier. In reality, the shell is where everything begins and ends. The Machine layer is the pure-computation core, not the top of a hierarchy.

Each boundary between layers is a **dual aspect functor** — it translates both structure and meaning in both directions:
- **Natural → Fluid:** Intent compilation (user says "find bottleneck" → fluid picks Laplacian)
- **Fluid → Machine:** Expression compilation (fluid picks algorithm → machine computes eigenvalue)
- **Machine → Fluid:** Result extraction (eigenvalue → fluid decides what it means)
- **Fluid → Natural:** Explanation extraction (fluid builds sentence → shell presents to human)

See [`THREE_LAYER_ARCHITECTURE.md`](THREE_LAYER_ARCHITECTURE.md) and [`docs/CORRECTED_MODEL.md`](docs/CORRECTED_MODEL.md) for the full architectural model.


### Enabling Features

```bash
# Build with mathematical analysis only
cargo build --features math-tools

# Build with griot history only
cargo build --features griot-history

# Build with everything
cargo build --features math-tools,griot-history

# Or use the convenience flag
cargo build --features all
```

Features can also be configured at runtime via `modules.toml`:

```toml
# ~/.config/intelligent-terminal/modules.toml

[math-tools]
enabled = true

[math-tools.verification_entropy]
alpha = 0.005
red_threshold = 0.8

[math-tools.hodge]
enabled = true

[griot-history]
enabled = true
decay_rate = 0.1
```

### Coming Soon

- **Context Triggers** — modules that activate based on what you're doing, not what you configured
- **Module Registry** — discoverable, pluggable modules with a standard trait interface (`TerminalModule`)
- **Spectral Dashboard** — live TUI rendering of agent collaboration networks with Fiedler values and Cheeger constants
- **Sheaf-Theoretic Error Analysis** — when multiple agents disagree, compute cohomology to determine if the disagreement is structural or resolvable
- **Free Probability Model Selection** — predict which LLM handles your prompt best using Marchenko-Pastur spectral analysis of attention patterns
- **Renormalization for Command History** — multi-scale skill analysis that detects learning plateaus and suggests breakthroughs

---

## Installing and running Intelligent Terminal

> [!NOTE]
> Intelligent Terminal requires Windows 11 22H2 or later (22621.6060+). You also need a supported agent CLI and subscription. [GitHub Copilot](https://github.com/features/copilot/cli/) is the default.

### Microsoft Store (recommended)

Install the [Intelligent Terminal from the Microsoft Store](https://apps.microsoft.com/detail/9NMQC2SSJX24).
This allows you to always be on the latest version when we release new builds
with automatic upgrades.

### WinGet

[winget](https://github.com/microsoft/winget-cli) users can download and install
the latest Intelligent Terminal release by installing the `Microsoft.IntelligentTerminal`
package:

```powershell
winget install --id Microsoft.IntelligentTerminal -e
```

### Downloads

| Distribution | Architecture | Link |
|--------------|:------------:|------|
| App Installer | x64, arm64, x86 | [Download](https://github.com/microsoft/intelligent-terminal/releases/latest) |


---

## Get Started

1. On first launch, choose your agent. Intelligent Terminal auto-detects several [ACP-compatible](https://agentclientprotocol.com/get-started/agents) agent CLIs on your machine (Copilot/Claude/Codex/Gemini). If none are found, it defaults to GitHub Copilot CLI and installs it for you via WinGet.
3. If you aren't already authenticated, the agent pane walks you through sign-in.
4. Start asking questions and using the agent pane for assistance. The agent has context on your shell output, no copy-pasting needed.

> [!TIP]
> If you see "running scripts is disabled on this system" or an `UnauthorizedAccess` error in PowerShell, your execution policy is blocking your profile and Intelligent Terminal can't initialize shell integration. Run:
> ```powershell
> Set-ExecutionPolicy -Scope CurrentUser -ExecutionPolicy RemoteSigned
> ```
> If you run into any other issues or dependency errors, see [installing-dependencies.md](./doc/installing-dependencies.md).

---

## Keyboard Shortcuts

All shortcuts are customizable through Intelligent Terminal settings.

| Shortcut | Action |
|----------|--------|
| <kbd>Ctrl+Shift+.</kbd> | Toggle the agent pane |
| <kbd>Ctrl+Shift+I</kbd> | Switch focus to/from the agent pane |
| <kbd>Ctrl+Alt+.</kbd> | Open agent pane with error context |
| <kbd>Ctrl+Shift+/</kbd> | Open agent management |
| <kbd>Alt+Shift+/</kbd> | Open Command Palette in prompt mode |
| <kbd>Alt+Shift+B</kbd> | Open an interactive delegate-agent tab with no startup prompt |

---

## Configuration

Everything is configurable through Intelligent Terminal settings, under "Agent" settings.

| Setting | Options |
|---------|---------|
| Agent and model | GitHub Copilot (default), or any ACP-compatible agent CLI, including custom or local agents. Configurable for both the agent pane and command palette. |
| Pane placement | Top, Bottom (default), Left, Right |
| Error detection | Allows Intelligent Terminal to automatically detect command failures |
| Error suggestions | Allows Intelligent Terminal to automatically send detected errors to the agent for fix suggestions |
| Agent session tracking (hooks) | Allows Intelligent Terminal to track active agent sessions and their status in the session management UI |

---

## Features

### Agent Status Bar

<p align="center">
  <img src="./images/intelligent-terminal-status-bar.png" alt="Screenshot of the agent status bar at the bottom of the terminal window">
</p>

The agent status bar sits at the bottom of the window and gives you quick access to everything agent-related. On the left: the agent pane toggle (hotkey: <kbd>Ctrl+Shift+.</kbd>) and the error detection icon (hotkey: <kbd>Ctrl+Alt+.</kbd>), which lights up when a fixable error is detected. On the right: the agent management icon (hotkey: <kbd>Ctrl+Shift+/</kbd>) that opens your session management panel. It's a persistent, minimal control surface so you're never more than one click away from your agents.

### Agent Pane

<p align="center">
  <img src="./images/intelligent-terminal-agent-pane.png" alt="Screenshot of the agent pane with a development conversation">
</p>

A context-aware, docked pane with your agent CLI of choice. The pane has context on your shell output across all your shells. Toggle with <kbd>Ctrl+Shift+.</kbd>, switch focus with <kbd>Ctrl+Shift+I</kbd>. If the agent needs to do multiple or complex tasks, it spins up background tasks in new tabs so your active shell stays focused.

<p align="center">
  <img src="./images/intelligent-terminal-agent-focus.png" alt="Screenshot of the agent pane with focus, showing multiple panes">
</p>

When you have multiple panes active, a small "Agent" indicator will appear on the pane that your agent has "focus" on.

### Agent Management

<p align="center">
  <img src="./images/intelligent-terminal-agent-management.png" alt="Screenshot of agent management panel showing active agents and past sessions">
</p>

View all active agents, their status, and past sessions. Pick up a workflow where you left off or check on a long-running task. Click the agent management icon in the status bar or press <kbd>Ctrl+Shift+/</kbd> to open it.

### Error Detection

<p align="center">
  <img src="./images/intelligent-terminal-error-detection.png" alt="Screenshot of automatic error detection with a suggested fix">
</p>

When a command fails, an indicator appears in the agent status bar. Click it or press <kbd>Ctrl+Alt+.</kbd> to open the agent pane with the error context already loaded. The agent can explain what happened and suggest or run a fix. Configure your settings to auto-detect errors only, or to also auto-suggest fixes.

### Command Palette

<p align="center">
  <img src="./images/intelligent-terminal-command-palette.png" alt="Screenshot of Command Palette with an agent prompt">
</p>

Type `?` followed by your prompt in the Command Palette to kick off an agent task. Intelligent Terminal injects context from the active pane and starts the agent in a background tab. Use <kbd>Alt+Shift+/</kbd> to jump directly into prompt mode.

---

## Data & Privacy

Intelligent Terminal is a **local transport layer**. It passes your prompts and shell context to your selected agent CLI over stdio/ACP. Intelligent Terminal does not call any cloud APIs itself and does not persist conversation history, however, diagnostic logs may be written to disk and telemetry may be emitted as described below.

### What data flows through Terminal

- Your prompts (what you type in the agent pane or command palette)
- Shell output context (recent command output shared with the agent for context)
- Basic environment metadata (shell type, OS version)

All of this is held in memory for the active session only and discarded when the session ends.

### Where your data goes depends on your agent CLI

| Agent CLI | Data routing | Terms |
|-----------|--------------|-------|
| [GitHub Copilot](https://github.com/features/copilot/cli/) (default) | GitHub backend | [GitHub Copilot Trust Center](https://resources.github.com/copilot-trust-center/). Enterprise protections (e.g., zero data retention) apply for eligible plans. |
| Third-party or custom agent CLIs | Determined by the agent vendor | Governed by that vendor's terms, not Microsoft or GitHub agreements. |

> [!NOTE]
> Terminal cannot guarantee data protections for third-party agent CLIs. When you select an agent, you're choosing where your data goes. Review your agent vendor's privacy policy before use. For more information on how to use GitHub Copilot responsibly, see [Responsible use of GitHub Copilot](https://docs.github.com/en/copilot/responsible-use/copilot-in-windows-terminal).

### Controls

- Choose your agent CLI at any time in Settings > Agent
- Disable auto error detection to prevent shell output from being detected automatically
- Intelligent Terminal always asks before running commands on your behalf in your shell

Intelligent Terminal only collects usage data and sends it to Microsoft to help improve our products and services. Read our [privacy statement](https://go.microsoft.com/fwlink/?LinkID=824704) to learn more. See [PRIVACY.md](./PRIVACY.md) for details and instructions on how to disable telemetry.

### SuperInstance Privacy

All SuperInstance modules are **pure-local computation**. No network calls, no external services, no API calls. The math — Hodge decomposition, spectral graph theory, ergodic Markov chains, exponential decay — runs locally on your machine. Your command history never leaves your terminal. The verification entropy tracker, error decomposition, and griot persistence models are all computed in-process and discarded on exit.

### Data Collection

The software may collect information about you and your use of the software and send it to Microsoft. Microsoft may use this information to provide services and improve our products and services. You may turn off the telemetry as described in the repository. There are also some features in the software that may enable you and Microsoft to collect data from users of your applications. If you use these features, you must comply with applicable law, including providing appropriate notices to users of your applications together with a copy of Microsoft's privacy statement. Our privacy statement is located at https://go.microsoft.com/fwlink/?LinkID=824704. You can learn more about data collection and use in the help documentation and our privacy statement. Your use of the software operates as your consent to these practices.

---

## Building the Code

Building Intelligent Terminal is the same as building Windows Terminal. See the [Developer Guidance](https://github.com/microsoft/terminal#developer-guidance) section of the Windows Terminal README for prerequisites, build instructions, and debugging steps.

To build with SuperInstance enhancements:

```bash
# Standard build (no enhancements)
cargo build

# With math-aware analysis
cargo build --features math-tools

# With griot command history
cargo build --features griot-history

# Everything
cargo build --features math-tools,griot-history
```

---

## Feedback

Intelligent Terminal is in an experimental stage. If you have a feature request or find a bug, [submit an issue](https://github.com/microsoft/intelligent-terminal/issues) on the GitHub repository. When filing a bug, the **Report a bug (collect logs)** command in the Command Palette (<kbd>Ctrl+Shift+P</kbd>) bundles your diagnostic logs into a timestamped ZIP on your Desktop — attach it to the issue so we have full context.

Intelligent Terminal ships as a separate app and installs next to your existing Windows Terminal. If you don't want agents in your terminal, nothing changes for you. With this model, we can learn, experiment, and iterate with you, the community, on what this evolution might look like without breaking your existing Windows Terminal flows.

---

## Contributing

We are excited to work alongside you, our amazing community, to build and enhance Intelligent Terminal!

**Before you start work on a feature/fix**, please read & follow the [Windows Terminal Contributor's Guide](https://github.com/microsoft/terminal/blob/main/CONTRIBUTING.md). The contribution process is the same.

---

## Code of Conduct

This project has adopted the [Microsoft Open Source Code of Conduct](https://opensource.microsoft.com/codeofconduct/). For more information, see the [Code of Conduct FAQ](https://opensource.microsoft.com/codeofconduct/faq/) or contact [opencode@microsoft.com](mailto:opencode@microsoft.com) with any additional questions or comments.

---

## Security

If you believe you have found a security vulnerability in this repository, please report it following the instructions in [SECURITY.md](./SECURITY.md).

---

## Trademarks

This project may contain trademarks or logos for projects, products, or services. Authorized use of Microsoft trademarks or logos is subject to and must follow [Microsoft's Trademark & Brand Guidelines](https://www.microsoft.com/en-us/legal/intellectualproperty/trademarks/usage/general). Use of Microsoft trademarks or logos in modified versions of this project must not cause confusion or imply Microsoft sponsorship. Any use of third-party trademarks or logos is subject to those third-party's policies.
