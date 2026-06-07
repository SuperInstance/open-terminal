//! Agent Overlay — Terminal overlay for conservation budget, fleet health, and spectral ranking.
//!
//! Renders real-time telemetry from the agent fleet as a terminal overlay.
//! Shows conservation budget status, fleet health indicators, and spectral
//! ranking of active agents/tasks.
//!
//! The terminal IS the agent interface. The agent authenticates via
//! lattice-crypto, shows real-time telemetry, and suggests commands
//! based on conservation state.

/// Budget state for rendering.
#[derive(Debug, Clone)]
pub struct BudgetState {
    /// Maximum budget capacity.
    pub max_budget: f64,
    /// Currently consumed budget.
    pub consumed: f64,
    /// Number of active tasks.
    pub active_tasks: usize,
    /// Number of pending tasks.
    pub pending_tasks: usize,
}

/// Fleet health status for a single agent.
#[derive(Debug, Clone)]
pub struct AgentHealth {
    /// Agent identifier.
    pub agent_id: String,
    /// Health score [0, 1].
    pub health: f64,
    /// Current spectral rank.
    pub spectral_rank: f64,
    /// Number of tasks this agent is handling.
    pub task_count: usize,
    /// Whether this agent is currently active.
    pub is_active: bool,
}

/// Overall fleet health.
#[derive(Debug, Clone)]
pub struct FleetHealth {
    /// Individual agent healths.
    pub agents: Vec<AgentHealth>,
    /// Average fleet health [0, 1].
    pub average_health: f64,
    /// Total budget utilization [0, 1].
    pub budget_utilization: f64,
    /// Spectral ranking of the fleet (dominant eigenvalue).
    pub fleet_spectral_rank: f64,
}

/// Output from rendering an overlay.
#[derive(Debug, Clone)]
pub struct RenderOutput {
    /// Rendered text content.
    pub content: String,
    /// Width of the rendered content.
    pub width: usize,
    /// Height of the rendered content.
    pub height: usize,
}

/// The agent overlay renderer.
pub struct AgentOverlay {
    /// Width of the terminal (characters).
    pub width: usize,
    /// Whether to show detailed agent info.
    pub verbose: bool,
}

impl AgentOverlay {
    /// Create a new overlay renderer with the given terminal width.
    pub fn new(width: usize) -> Self {
        Self {
            width,
            verbose: false,
        }
    }

    /// Enable verbose output.
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Render the conservation budget display.
    ///
    /// Shows a bar chart of budget consumption with numerical indicators.
    pub fn render_budget(&self, budget: &BudgetState) -> RenderOutput {
        let bar_width = self.width.saturating_sub(30).min(40).max(10);
        let utilization = if budget.max_budget > 0.0 {
            budget.consumed / budget.max_budget
        } else {
            0.0
        };

        let filled = (utilization * bar_width as f64).round() as usize;
        let empty = bar_width.saturating_sub(filled);

        let bar: String = format!(
            "{}{}",
            "█".repeat(filled),
            "░".repeat(empty)
        );

        let pct = (utilization * 100.0).min(100.0);

        let mut lines = Vec::new();
        lines.push(format!(
            "╭─ Conservation Budget ─────────────────╮"
        ));
        lines.push(format!(
            "│ [{bar}] {pct:5.1}% │",
            bar = bar,
            pct = pct
        ));
        lines.push(format!(
            "│ Consumed: {consumed:8.2} / {max:8.2}       │",
            consumed = budget.consumed,
            max = budget.max_budget
        ));
        lines.push(format!(
            "│ Active: {active:3}  Pending: {pending:3}          │",
            active = budget.active_tasks,
            pending = budget.pending_tasks
        ));
        lines.push(format!(
            "╰───────────────────────────────────────╯"
        ));

        let content = lines.join("\n");
        RenderOutput {
            height: lines.len(),
            width: self.width,
            content,
        }
    }

    /// Render the fleet health display.
    ///
    /// Shows each agent's health as a mini-bar, plus overall fleet statistics.
    pub fn render_fleet(&self, health: &FleetHealth) -> RenderOutput {
        let mut lines = Vec::new();
        lines.push(format!(
            "╭─ Fleet Health ────────────────────────╮"
        ));

        if health.agents.is_empty() {
            lines.push(format!(
                "│  (no agents connected)                │"
            ));
        } else {
            let display_agents: Vec<&AgentHealth> = if self.verbose {
                health.agents.iter().collect()
            } else {
                health.agents.iter().take(5).collect()
            };

            for agent in display_agents {
                let status_icon = if agent.is_active { "●" } else { "○" };
                let health_bar_len: usize = 10;
                let health_filled = (agent.health * health_bar_len as f64).round() as usize;
                let health_empty = health_bar_len.saturating_sub(health_filled);
                let health_bar = format!(
                    "{}{}",
                    "▓".repeat(health_filled),
                    "░".repeat(health_empty)
                );

                lines.push(format!(
                    "│ {icon} {id:<12} [{bar}] {h:.0}% T:{tasks:2} │",
                    icon = status_icon,
                    id = truncate_str(&agent.agent_id, 12),
                    bar = health_bar,
                    h = agent.health * 100.0,
                    tasks = agent.task_count,
                ));
            }

            if !self.verbose && health.agents.len() > 5 {
                lines.push(format!(
                    "│   ... and {} more agents             │",
                    health.agents.len() - 5
                ));
            }
        }

        lines.push(format!(
            "├───────────────────────────────────────┤"
        ));
        lines.push(format!(
            "│ Fleet Health: {avg:5.1}%  Budget: {bud:5.1}%   │",
            avg = health.average_health * 100.0,
            bud = health.budget_utilization * 100.0,
        ));
        lines.push(format!(
            "│ Spectral Rank: {rank:.4}               │",
            rank = health.fleet_spectral_rank,
        ));
        lines.push(format!(
            "╰───────────────────────────────────────╯"
        ));

        let content = lines.join("\n");
        RenderOutput {
            height: lines.len(),
            width: self.width,
            content,
        }
    }

    /// Render a combined dashboard with both budget and fleet.
    pub fn render_dashboard(
        &self,
        budget: &BudgetState,
        fleet: &FleetHealth,
    ) -> RenderOutput {
        let budget_render = self.render_budget(budget);
        let fleet_render = self.render_fleet(fleet);

        let content = format!(
            "{budget}\n{fleet}",
            budget = budget_render.content,
            fleet = fleet_render.content,
        );

        RenderOutput {
            height: budget_render.height + fleet_render.height,
            width: self.width,
            content,
        }
    }
}

/// Truncate a string to a maximum length, adding "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        format!("{:<width$}", s, width = max_len)
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_budget_zero_consumption() {
        let overlay = AgentOverlay::new(80);
        let budget = BudgetState {
            max_budget: 100.0,
            consumed: 0.0,
            active_tasks: 0,
            pending_tasks: 0,
        };
        let output = overlay.render_budget(&budget);
        assert!(output.content.contains("0.0%"));
        assert!(output.height > 0);
    }

    #[test]
    fn test_render_budget_full_consumption() {
        let overlay = AgentOverlay::new(80);
        let budget = BudgetState {
            max_budget: 100.0,
            consumed: 100.0,
            active_tasks: 5,
            pending_tasks: 3,
        };
        let output = overlay.render_budget(&budget);
        assert!(output.content.contains("100.0%"));
        assert!(output.content.contains("Active:   5"));
        assert!(output.content.contains("Pending:   3"));
    }

    #[test]
    fn test_render_budget_partial() {
        let overlay = AgentOverlay::new(80);
        let budget = BudgetState {
            max_budget: 200.0,
            consumed: 50.0,
            active_tasks: 2,
            pending_tasks: 1,
        };
        let output = overlay.render_budget(&budget);
        assert!(output.content.contains("25.0%"));
    }

    #[test]
    fn test_render_fleet_empty() {
        let overlay = AgentOverlay::new(80);
        let fleet = FleetHealth {
            agents: vec![],
            average_health: 0.0,
            budget_utilization: 0.0,
            fleet_spectral_rank: 0.0,
        };
        let output = overlay.render_fleet(&fleet);
        assert!(output.content.contains("no agents"));
    }

    #[test]
    fn test_render_fleet_with_agents() {
        let overlay = AgentOverlay::new(80);
        let fleet = FleetHealth {
            agents: vec![
                AgentHealth {
                    agent_id: "agent-1".into(),
                    health: 0.95,
                    spectral_rank: 0.8,
                    task_count: 3,
                    is_active: true,
                },
                AgentHealth {
                    agent_id: "agent-2".into(),
                    health: 0.70,
                    spectral_rank: 0.5,
                    task_count: 1,
                    is_active: false,
                },
            ],
            average_health: 0.825,
            budget_utilization: 0.6,
            fleet_spectral_rank: 0.75,
        };
        let output = overlay.render_fleet(&fleet);
        assert!(output.content.contains("agent-1"));
        assert!(output.content.contains("agent-2"));
        assert!(output.content.contains("●"));
        assert!(output.content.contains("○"));
        assert!(output.content.contains("82.5%"));
    }

    #[test]
    fn test_render_fleet_verbose() {
        let overlay = AgentOverlay::new(80).with_verbose(true);
        let mut agents = Vec::new();
        for i in 0..8 {
            agents.push(AgentHealth {
                agent_id: format!("agent-{}", i),
                health: 0.9,
                spectral_rank: 0.7,
                task_count: 2,
                is_active: true,
            });
        }
        let fleet = FleetHealth {
            agents,
            average_health: 0.9,
            budget_utilization: 0.5,
            fleet_spectral_rank: 0.8,
        };
        let output = overlay.render_fleet(&fleet);
        // Verbose should show all 8 agents
        for i in 0..8 {
            assert!(output.content.contains(&format!("agent-{}", i)));
        }
    }

    #[test]
    fn test_render_fleet_truncated() {
        let overlay = AgentOverlay::new(80);
        let mut agents = Vec::new();
        for i in 0..8 {
            agents.push(AgentHealth {
                agent_id: format!("agent-{}", i),
                health: 0.9,
                spectral_rank: 0.7,
                task_count: 2,
                is_active: true,
            });
        }
        let fleet = FleetHealth {
            agents,
            average_health: 0.9,
            budget_utilization: 0.5,
            fleet_spectral_rank: 0.8,
        };
        let output = overlay.render_fleet(&fleet);
        // Non-verbose should only show first 5 and mention "more agents"
        assert!(output.content.contains("3 more agents"));
    }

    #[test]
    fn test_render_dashboard() {
        let overlay = AgentOverlay::new(80);
        let budget = BudgetState {
            max_budget: 100.0,
            consumed: 50.0,
            active_tasks: 3,
            pending_tasks: 2,
        };
        let fleet = FleetHealth {
            agents: vec![AgentHealth {
                agent_id: "main".into(),
                health: 0.95,
                spectral_rank: 0.9,
                task_count: 3,
                is_active: true,
            }],
            average_health: 0.95,
            budget_utilization: 0.5,
            fleet_spectral_rank: 0.9,
        };
        let output = overlay.render_dashboard(&budget, &fleet);
        assert!(output.content.contains("Conservation Budget"));
        assert!(output.content.contains("Fleet Health"));
        assert!(output.height > 10);
    }

    #[test]
    fn test_truncate_str_short() {
        assert_eq!(truncate_str("hi", 5), "hi   ");
    }

    #[test]
    fn test_truncate_str_long() {
        let result = truncate_str("hello world", 8);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_render_budget_zero_max() {
        let overlay = AgentOverlay::new(80);
        let budget = BudgetState {
            max_budget: 0.0,
            consumed: 0.0,
            active_tasks: 0,
            pending_tasks: 0,
        };
        let output = overlay.render_budget(&budget);
        assert!(output.content.contains("0.0%"));
    }
}
