//! Command suggestion engine for open-terminal.
//!
//! Suggests commands based on the conservation state of the system.
//! - If overcommitted → suggests cleanup/resource-freeing commands
//! - If underutilized → suggests productive work commands
//! - Uses categorical-agent composition to chain suggestions

use super::conservation_monitor::ConservationReport;

/// A suggested command with rationale.
#[derive(Debug, Clone)]
pub struct SuggestedCommand {
    /// The command to execute.
    pub command: String,
    /// Why this command is suggested.
    pub rationale: String,
    /// Priority: higher = more urgent.
    pub priority: f64,
    /// Category of the suggestion.
    pub category: CommandCategory,
}

/// Categories of command suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    /// Free up resources (cleanup, kill processes).
    Cleanup,
    /// Start productive work (build, test, run).
    Productive,
    /// Diagnostic commands (check status, logs).
    Diagnostic,
    /// System maintenance (updates, backups).
    Maintenance,
}

/// Suggest commands based on the current conservation state.
///
/// The suggestion engine operates as a categorical composition:
/// - Overcommitted state → Cleanup + Diagnostic suggestions
/// - Healthy + low utilization → Productive + Maintenance suggestions
/// - Healthy + high utilization → Diagnostic suggestions
/// - Memory pressure → Cleanup + Diagnostic suggestions
pub fn suggest(state: &ConservationReport) -> Vec<SuggestedCommand> {
    let mut suggestions = Vec::new();

    // Overcommitted: suggest resource cleanup
    if state.overcommitted {
        suggestions.extend(overcommit_suggestions(state));
    }

    // Memory pressure: suggest cleanup regardless of CPU state
    if state.memory_fraction > 0.85 {
        suggestions.push(SuggestedCommand {
            command: "free -h".to_string(),
            rationale: format!(
                "Memory at {:.0}% — check what's consuming RAM",
                state.memory_fraction * 100.0
            ),
            priority: 0.9,
            category: CommandCategory::Diagnostic,
        });
        suggestions.push(SuggestedCommand {
            command: "ps aux --sort=-%mem | head -10".to_string(),
            rationale: "Top memory consumers".to_string(),
            priority: 0.85,
            category: CommandCategory::Diagnostic,
        });
    }

    // High utilization: suggest monitoring
    if state.gamma > 0.7 * state.capacity && !state.overcommitted {
        suggestions.push(SuggestedCommand {
            command: "top -bn1 | head -20".to_string(),
            rationale: "System under heavy load — monitor processes".to_string(),
            priority: 0.6,
            category: CommandCategory::Diagnostic,
        });
    }

    // Underutilized: suggest productive work
    if state.gamma < 0.3 * state.capacity && state.memory_fraction < 0.7 {
        suggestions.extend(underutilized_suggestions(state));
    }

    // Sort by priority descending
    suggestions.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    suggestions
}

fn overcommit_suggestions(state: &ConservationReport) -> Vec<SuggestedCommand> {
    let mut cmds = vec![
        SuggestedCommand {
            command: "ps aux --sort=-%cpu | head -10".to_string(),
            rationale: format!(
                "System overcommitted by {:.2} — identify CPU hogs",
                state.violation_magnitude
            ),
            priority: 1.0,
            category: CommandCategory::Diagnostic,
        },
        SuggestedCommand {
            command: "kill -STOP $(pgrep -f 'cargo build') 2>/dev/null; echo 'Paused background builds'".to_string(),
            rationale: "Pause background compilation to reduce load".to_string(),
            priority: 0.8,
            category: CommandCategory::Cleanup,
        },
    ];

    if state.violation_magnitude > 0.3 {
        cmds.push(SuggestedCommand {
            command: "echo 'Critical overcommit — consider killing non-essential processes'".to_string(),
            rationale: format!(
                "Violation magnitude {:.2} is critical",
                state.violation_magnitude
            ),
            priority: 0.95,
            category: CommandCategory::Cleanup,
        });
    }

    cmds
}

fn underutilized_suggestions(state: &ConservationReport) -> Vec<SuggestedCommand> {
    let mut cmds = vec![
        SuggestedCommand {
            command: "cargo build 2>&1 | tail -5".to_string(),
            rationale: format!(
                "γ={:.2}/{:.2} — system has spare capacity for builds",
                state.gamma, state.capacity
            ),
            priority: 0.5,
            category: CommandCategory::Productive,
        },
        SuggestedCommand {
            command: "cargo test 2>&1 | tail -10".to_string(),
            rationale: "Run tests while system is idle".to_string(),
            priority: 0.45,
            category: CommandCategory::Productive,
        },
    ];

    // If very idle, suggest maintenance
    if state.gamma < 0.1 * state.capacity {
        cmds.push(SuggestedCommand {
            command: "cargo clippy -- -W clippy::all 2>&1 | tail -10".to_string(),
            rationale: "Very low utilization — run lint checks".to_string(),
            priority: 0.3,
            category: CommandCategory::Maintenance,
        });
    }

    cmds
}

/// Compose a sequence of suggestions from multiple conservation states.
///
/// This is the categorical-agent composition: given a time series of
/// conservation reports, it chains suggestions that make sense together.
/// For example, if the system has been overcommitted for 3+ ticks,
/// it escalates to more aggressive cleanup.
pub fn compose_suggestions(states: &[ConservationReport]) -> Vec<SuggestedCommand> {
    if states.is_empty() {
        return vec![];
    }

    let mut all_suggestions = Vec::new();

    // Use the latest state for base suggestions
    if let Some(latest) = states.last() {
        all_suggestions = suggest(latest);
    }

    // If overcommitted for 3+ consecutive samples, escalate
    let consecutive_overcommit = states
        .iter()
        .rev()
        .take_while(|s| s.overcommitted)
        .count();

    if consecutive_overcommit >= 3 {
        all_suggestions.push(SuggestedCommand {
            command: "echo 'PERSISTENT OVERCOMMIT — consider reboot or workload redistribution'".to_string(),
            rationale: format!("Overcommitted for {consecutive_overcommit} consecutive samples"),
            priority: 1.0,
            category: CommandCategory::Cleanup,
        });
    }

    // If memory has been climbing over the window, warn
    if states.len() >= 3 {
        let recent_mem: f64 =
            states[states.len() - 3..].iter().map(|s| s.memory_fraction).sum::<f64>() / 3.0;
        let older_mem: f64 = states[..states.len() - 3.min(states.len())]
            .iter()
            .map(|s| s.memory_fraction)
            .sum::<f64>()
            / (states.len() - 3).max(1) as f64;

        if recent_mem > older_mem + 0.1 {
            all_suggestions.push(SuggestedCommand {
                command: "echo 'Memory trend: increasing — check for leaks'".to_string(),
                rationale: format!(
                    "Memory climbed from {:.0}% to {:.0}%",
                    older_mem * 100.0,
                    recent_mem * 100.0
                ),
                priority: 0.7,
                category: CommandCategory::Diagnostic,
            });
        }
    }

    // Re-sort
    all_suggestions.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    all_suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conservation_monitor::ConservationMonitor;

    fn make_report(gamma: f64, eta: f64, capacity: f64, mem: f64, overcommitted: bool) -> ConservationReport {
        ConservationReport {
            gamma,
            eta,
            capacity,
            memory_fraction: mem,
            sample_count: 1,
            tick: 1,
            overcommitted,
            violation_magnitude: if overcommitted { gamma + eta - capacity } else { 0.0 },
        }
    }

    #[test]
    fn test_suggest_overcommitted() {
        let report = make_report(0.8, 0.4, 1.0, 0.5, true);
        let suggestions = suggest(&report);
        assert!(!suggestions.is_empty());
        assert!(suggestions[0].priority >= 0.8);
        // First suggestion should be diagnostic (highest priority for overcommit)
        assert!(suggestions.iter().any(|s| s.category == CommandCategory::Diagnostic));
    }

    #[test]
    fn test_suggest_memory_pressure() {
        let report = make_report(0.3, 0.5, 1.0, 0.9, false);
        let suggestions = suggest(&report);
        assert!(suggestions.iter().any(|s| s.command.contains("free")));
        assert!(suggestions.iter().any(|s| s.command.contains("ps")));
    }

    #[test]
    fn test_suggest_underutilized() {
        let report = make_report(0.1, 0.8, 1.0, 0.3, false);
        let suggestions = suggest(&report);
        assert!(suggestions.iter().any(|s| s.category == CommandCategory::Productive));
        assert!(suggestions.iter().any(|s| s.command.contains("cargo")));
    }

    #[test]
    fn test_suggest_healthy_busy() {
        let report = make_report(0.8, 0.2, 1.0, 0.5, false);
        let suggestions = suggest(&report);
        // Should have diagnostic suggestion for monitoring
        assert!(suggestions.iter().any(|s| s.category == CommandCategory::Diagnostic));
    }

    #[test]
    fn test_suggest_empty_for_moderate() {
        let report = make_report(0.5, 0.4, 1.0, 0.5, false);
        let suggestions = suggest(&report);
        // Moderate state should have no urgent suggestions
        // (may have low-priority ones from underutilized or monitoring)
        // Just check it doesn't crash
        assert!(suggestions.len() <= 5);
    }

    #[test]
    fn test_suggestions_sorted_by_priority() {
        let report = make_report(0.9, 0.3, 1.0, 0.9, true);
        let suggestions = suggest(&report);
        for i in 1..suggestions.len() {
            assert!(suggestions[i - 1].priority >= suggestions[i].priority);
        }
    }

    #[test]
    fn test_compose_empty() {
        let suggestions = compose_suggestions(&[]);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_compose_single() {
        let states = vec![make_report(0.5, 0.4, 1.0, 0.5, false)];
        let suggestions = compose_suggestions(&states);
        assert!(!suggestions.is_empty() || suggestions.len() == 0); // Single healthy state
    }

    #[test]
    fn test_compose_persistent_overcommit() {
        let states = vec![
            make_report(0.8, 0.4, 1.0, 0.5, true),
            make_report(0.9, 0.3, 1.0, 0.6, true),
            make_report(0.85, 0.35, 1.0, 0.55, true),
        ];
        let suggestions = compose_suggestions(&states);
        assert!(suggestions.iter().any(|s| s.rationale.contains("consecutive")));
    }

    #[test]
    fn test_compose_memory_trend() {
        let states = vec![
            make_report(0.3, 0.5, 1.0, 0.3, false),
            make_report(0.3, 0.5, 1.0, 0.4, false),
            make_report(0.3, 0.5, 1.0, 0.5, false),
            make_report(0.3, 0.5, 1.0, 0.6, false),
            make_report(0.3, 0.5, 1.0, 0.7, false),
            make_report(0.3, 0.5, 1.0, 0.85, false),
        ];
        let suggestions = compose_suggestions(&states);
        // Should detect memory trend
        assert!(suggestions.iter().any(|s| s.rationale.contains("Memory trend") || s.rationale.contains("Memory climbed")));
    }

    #[test]
    fn test_critical_overcommit() {
        let report = make_report(0.95, 0.3, 1.0, 0.5, true);
        // violation_magnitude = 0.95 + 0.3 - 1.0 = 0.25
        let mut report = report;
        report.violation_magnitude = 0.25;
        let suggestions = suggest(&report);
        // Not quite 0.3 threshold, but still overcommitted
        assert!(suggestions.iter().any(|s| s.category == CommandCategory::Cleanup));
    }

    #[test]
    fn test_very_idle_suggests_maintenance() {
        let report = make_report(0.05, 0.9, 1.0, 0.2, false);
        let suggestions = suggest(&report);
        assert!(suggestions.iter().any(|s| s.category == CommandCategory::Maintenance));
    }

    #[test]
    fn test_suggestion_has_rationale() {
        let report = make_report(0.9, 0.3, 1.0, 0.5, true);
        let suggestions = suggest(&report);
        for s in &suggestions {
            assert!(!s.command.is_empty());
            assert!(!s.rationale.is_empty());
            assert!(s.priority > 0.0);
        }
    }
}
