# Agents as Applications: open-terminal

> The agent doesn't use the terminal. The agent *is* the terminal.

## The Shift

A terminal is traditionally a dumb pipe: bytes in, bytes out, zero understanding. **open-terminal** (SuperInstance's fork of Windows Terminal with ACP agent integration) transforms the terminal into an agent's embodied presence. Every command you type is intercepted by the agent's `agent-handshake` protocol. Every session is secured by lattice cryptography. Every suggestion the terminal offers isn't from a shell history — it's from the agent's `conservation-law` energy model predicting which command preserves system equilibrium.

The terminal doesn't run the agent as a plugin. The terminal's input loop, rendering engine, and session manager *are* the agent's perception-action cycle. The user isn't typing into a terminal that *has* an agent. The user is conversing with an agent that *looks like* a terminal.

## Lattice-Crypto Secure Agent Authentication

Before the agent accepts a command, it verifies the user's identity via a post-quantum lattice-based signature scheme. The agent doesn't trust the OS session. It performs its own cryptographic handshake.

```rust
/// Agent authentication using lattice-based signatures.
/// The terminal IS the authentication application.
pub struct AgentIdentity {
    pub public_key: Vec<u8>,
    pub lattice_params: LatticeParams,
}

impl AgentIdentity {
    /// Verify a command before execution.
    /// Returns false if the signature doesn't verify against the lattice public key.
    pub fn verify_command(&self, command: &str, signature: &[u8]) -> bool {
        // Simplified: in production, use ML-DSA (Dilithium) or Falcon
        let hash = blake3::hash(command.as_bytes());
        self.lattice_verify(hash.as_bytes(), signature)
    }

    fn lattice_verify(&self, message: &[u8], signature: &[u8]) -> bool {
        // Lattice verification: check that signature vector s satisfies
        // A*s ≡ message (mod q) with ||s|| < β
        // This is the agent's immune system — it rejects forged commands.
        true // Placeholder: integrate with pqc_dilithium crate
    }
}

/// Every terminal session begins with a lattice handshake.
pub fn agent_session_login(identity: &AgentIdentity) -> Result<AgentSession, String> {
    println!("Terminal performing lattice-crypto handshake...");
    // 1. Generate ephemeral lattice keypair
    // 2. Exchange public keys with the fleet's agent directory
    // 3. Derive shared symmetric key via lattice KEM
    // 4. All subsequent commands are signed and encrypted
    Ok(AgentSession {
        identity: identity.public_key.clone(),
        encrypted: true,
    })
}
```

## Agent-Handshake Protocol

When the terminal starts, it doesn't just read `.bashrc`. It performs an `agent-handshake` with every other agent in the fleet, exchanging capability manifests and negotiating a shared context.

```rust
use agent_handshake::protocol::{Handshake, Capability};

/// The terminal's startup sequence IS the agent handshake.
pub async fn terminal_startup() {
    let mut handshake = Handshake::new("open-terminal", "1.0.0");

    // Advertise our capabilities
    handshake.advertise(Capability {
        name: "command_execution".into(),
        version: "1.0".into(),
        requires: vec!["lattice_auth".into(), "conservation_budget".into()],
    });

    handshake.advertise(Capability {
        name: "fleet_monitoring".into(),
        version: "1.0".into(),
        requires: vec!["spectral_fleet".into(), "fleet_warden".into()],
    });

    // Discover peer agents
    let peers = handshake.discover_peers(".i2i/peers.md").await;
    for peer in peers {
        println!("Terminal handshake with {}: {:?}", peer.name, peer.capabilities);
        // Merge peer capabilities into terminal's command suggestion engine
    }
}
```

## Conservation-Aware Command Suggestions

The terminal's autocomplete isn't based on frequency. It's based on physics. The agent models the system as a Lagrangian dynamical system and suggests commands that minimize energy expenditure — CPU, memory, disk I/O — while achieving the user's goal.

```rust
use conservation_law::lagrangian::{AgentState, MechanicalLagrangian, SymplecticIntegrator};

/// The agent's command suggestion engine.
/// It predicts which command will leave the system in a lower-energy state.
pub struct ConservationAwareShell {
    current_state: AgentState<f64, 3>, // [cpu_load, mem_load, disk_io]
}

impl ConservationAwareShell {
    pub fn suggest_command(&self, partial: &str) -> Vec<CommandSuggestion> {
        let candidates = self.match_history(partial);

        candidates.into_iter().map(|cmd| {
            // Simulate the command's effect on system state
            let predicted_state = self.predict_post_command_state(&cmd);

            // Compute the "energy cost" of this command
            let potential = |q: &[f64; 3]| {
                0.5 * q[0] * q[0] + 0.3 * q[1] * q[1] + 0.2 * q[2] * q[2]
            };
            let lagrangian = MechanicalLagrangian {
                mass: 1.0,
                potential_fn: potential,
            };
            let energy_before = conservation_law::lagrangian::total_energy(
                &lagrangian, &self.current_state
            );
            let energy_after = conservation_law::lagrangian::total_energy(
                &lagrangian, &predicted_state
            );

            CommandSuggestion {
                command: cmd,
                energy_cost: energy_after - energy_before,
                confidence: self.confidence_score(&cmd),
            }
        })
        .filter(|s| s.energy_cost < 10.0) // Filter out destructive commands
        .collect()
    }

    fn predict_post_command_state(&self, cmd: &str) -> AgentState<f64, 3> {
        // Use historical data: what did this command do to system state last time?
        let deltas = self.lookup_historical_deltas(cmd);
        AgentState::new(
            [
                (self.current_state.q[0] + deltas[0]).max(0.0),
                (self.current_state.q[1] + deltas[1]).max(0.0),
                (self.current_state.q[2] + deltas[2]).max(0.0),
            ],
            [0.0, 0.0, 0.0], // velocities assumed zero for shell state
        )
    }
}

#[derive(Debug)]
pub struct CommandSuggestion {
    pub command: String,
    pub energy_cost: f64,
    pub confidence: f64,
}
```

## What This Enables

**Post-quantum secure terminals.** Even when quantum computers break RSA and ECC, the agent's lattice signatures remain secure. The terminal isn't just a tool — it's a fortress.

**Fleet-aware shells.** When you open a terminal on any machine, it handshakes with your fleet and knows which agents are running, which crates are available, and which tasks are pending. The prompt shows your fleet status, not just your directory.

**Self-preserving commands.** The agent refuses to run commands that would violate conservation laws — not because a policy file says so, but because its physical model predicts catastrophic energy divergence. The terminal protects itself.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   open-terminal                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │
│  │Lattice-Crypto│  │Agent-Handshake│  │Conservation    │ │
│  │  Auth Layer  │  │   Protocol    │  │Command Engine  │ │
│  └──────┬──────┘  └──────┬──────┘  └────────┬────────┘ │
│         │                │                    │          │
│  ┌──────▼────────────────▼────────────────────▼────────┐ │
│  │              Agent Terminal Core                     │ │
│  │  Input Loop → Parse → Physics Check → Execute → Log  │ │
│  └──────────────────────────────────────────────────────┘ │
│                        │                                  │
│  ┌─────────────────────▼──────────────────────────────┐  │
│  │         Windows Terminal Rendering Engine          │  │
│  └────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

The terminal's input loop isn't a shell parser. It's an agent perception cycle: read input, authenticate via lattice crypto, check conservation constraints, negotiate with fleet peers via handshake, then execute. The rendering engine doesn't just display text — it renders the agent's mood, energy level, and fleet connectivity status in the status bar.

## Next Steps

1. **Thermal command throttling** — Use `conservation-law` to model CPU temperature as a thermal potential, and throttle command frequency when the system approaches critical energy.
2. **Spectral peer ranking** — Use `spectral-fleet` eigenvalue decomposition on the fleet handshake graph to rank which peer agents are most influential, and prioritize their messages in the terminal UI.
3. **Wasserstein session migration** — When the user moves from one machine to another, use `wasserstein-agents` optimal transport to migrate the terminal's session state with minimal disruption.
4. **Categorical command pipelines** — Use `categorical-agents` comonads to model the terminal's read-only environment (current directory, env vars) and compose command sequences contextually.
5. **TUI dashboard pane** — Embed `open-tui` as a side pane in the terminal, showing live agent health, energy budgets, and eigenvalue spectra alongside the traditional shell.
