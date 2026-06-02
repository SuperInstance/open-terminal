//! Context detection rules for the trigger engine.
//!
//! Every trigger is a pure `fn(&TerminalEvent) -> bool` — no state,
//! no IO, no allocation. This guarantees the trigger engine adds
//! <1ms latency to the event loop.

use super::{ProjectType, TerminalEvent};

/// ## math-tools trigger
///
/// Fires when:
/// 1. User runs `cargo test`, `python -m pytest`, or any test command.
/// 2. Multiple agent panes are open simultaneously (spectral dashboard context).
///
/// These indicate the user is in a development/testing workflow where
/// math-aware analysis adds value.
#[cfg(feature = "math-tools")]
pub fn math_tools_trigger(event: &TerminalEvent) -> bool {
    match event {
        // Test commands: cargo test, python -m pytest, etc.
        TerminalEvent::TestCommand { .. } => true,
        // Multiple agent panes indicate collaborative debugging context
        TerminalEvent::AgentPaneChanged { open_count } => *open_count >= 2,
        // Build commands that might benefit from math analysis
        TerminalEvent::BuildCompleted { .. } => true,
        _ => false,
    }
}

/// ## griot-history trigger
///
/// Always-on for experienced users. Fires when the user has >50 commands
/// in history. Below that threshold there isn't enough data for meaningful
/// pattern analysis.
#[cfg(feature = "griot-history")]
pub fn griot_history_trigger(event: &TerminalEvent) -> bool {
    match event {
        TerminalEvent::HistorySizeChanged { total_commands } => *total_commands >= 50,
        _ => false,
    }
}

/// ## griot-history PATTERN sub-trigger
///
/// Separate trigger for the pattern-mining sub-module. Only fires after
/// 200+ commands — before that there isn't enough data for reliable
/// workflow pattern detection.
#[cfg(feature = "griot-history")]
pub fn griot_history_pattern_trigger(event: &TerminalEvent) -> bool {
    match event {
        TerminalEvent::HistorySizeChanged { total_commands } => *total_commands >= 200,
        _ => false,
    }
}

/// ## verification-entropy trigger
///
/// Fires when a build command completes (`cargo build`, `npm run build`)
/// AND no test command has been run recently. This is the "you haven't
/// tested" warning. The "last N minutes" check is done at the module
/// level since it requires state (last test timestamp). The trigger
/// itself only checks for build completion.
///
/// Pure-trigger check: fires on any BuildCompleted event. The module
/// then decides whether the "no recent tests" condition is met.
#[cfg(feature = "math-tools")]
pub fn verification_entropy_trigger(event: &TerminalEvent) -> bool {
    match event {
        // Build completed — the module will check the "no recent tests" condition
        TerminalEvent::BuildCompleted { exit_code, .. } => *exit_code == 0,
        _ => false,
    }
}

/// ## error-hodge trigger
///
/// Fires when any command exits non-zero. Immediately triggers error
/// decomposition. Pure match on exit code — no analysis here.
#[cfg(feature = "math-tools")]
pub fn error_hodge_trigger(event: &TerminalEvent) -> bool {
    match event {
        TerminalEvent::CommandFailed { exit_code, .. } => *exit_code != 0,
        TerminalEvent::CommandExecuted { exit_code, .. } => *exit_code != 0,
        _ => false,
    }
}

/// ## spectral-dashboard trigger
///
/// Fires when 2+ agent panes are open and disagreeing on a fix.
/// Also a softer trigger: fires when 2+ panes are open in general,
/// since that's a pre-condition for disagreement.
#[cfg(feature = "math-tools")]
pub fn spectral_dashboard_trigger(event: &TerminalEvent) -> bool {
    match event {
        // Multiple panes — potential for disagreement
        TerminalEvent::AgentPaneChanged { open_count } => *open_count >= 2,
        // Explicit disagreement event
        TerminalEvent::AgentDisagreement { pane_count } => *pane_count >= 2,
        _ => false,
    }
}

/// ## adinkra trigger
///
/// Fires when a new project is detected (Cargo.toml created,
/// package.json created). Suggests aliases for that project type.
#[cfg(feature = "griot-history")]
pub fn adinkra_trigger(event: &TerminalEvent) -> bool {
    match event {
        TerminalEvent::ProjectFileCreated { project_type, .. } => {
            matches!(project_type, ProjectType::Rust | ProjectType::Node)
        }
        _ => false,
    }
}

/// ## command-markov trigger
///
/// Fires after 100+ commands to build the transition matrix.
/// Then fires anomaly detection on every subsequent CommandExecuted event.
#[cfg(feature = "math-tools")]
pub fn command_markov_trigger(event: &TerminalEvent) -> bool {
    match event {
        // Initial trigger: enough data to build the transition matrix
        TerminalEvent::HistorySizeChanged { total_commands } => *total_commands >= 100,
        // Subsequent trigger: every new command for anomaly detection
        TerminalEvent::CommandExecuted { .. } => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn math_tools_fires_on_test_variants() {
        #[cfg(feature = "math-tools")]
        {
            let variants = [
                TerminalEvent::TestCommand { text: "cargo test".into() },
                TerminalEvent::TestCommand { text: "python -m pytest".into() },
                TerminalEvent::TestCommand { text: "npm test".into() },
                TerminalEvent::TestCommand { text: "go test ./...".into() },
            ];
            for ev in &variants {
                assert!(math_tools_trigger(ev), "should fire on {:?}", ev);
            }
        }
    }

    #[test]
    fn verification_entropy_only_on_successful_builds() {
        #[cfg(feature = "math-tools")]
        {
            let fail = TerminalEvent::BuildCompleted {
                text: "cargo build".into(),
                exit_code: 1,
            };
            assert!(!verification_entropy_trigger(&fail), "should NOT fire on failed build");
            let ok = TerminalEvent::BuildCompleted {
                text: "cargo build".into(),
                exit_code: 0,
            };
            assert!(verification_entropy_trigger(&ok), "should fire on successful build");
        }
    }

    #[test]
    fn error_hodge_exit_code_exactness() {
        #[cfg(feature = "math-tools")]
        {
            let zero = TerminalEvent::CommandFailed { text: "true".into(), exit_code: 0 };
            assert!(!error_hodge_trigger(&zero), "exit 0 should NOT fire");
            let one = TerminalEvent::CommandFailed { text: "false".into(), exit_code: 1 };
            assert!(error_hodge_trigger(&one), "exit 1 should fire");
            let sig = TerminalEvent::CommandFailed { text: "kill -9".into(), exit_code: -9 };
            assert!(error_hodge_trigger(&sig), "negative exit should fire");
        }
    }

    #[test]
    fn command_markov_initial_trigger_is_history_size() {
        #[cfg(feature = "math-tools")]
        {
            let size_50 = TerminalEvent::HistorySizeChanged { total_commands: 50 };
            assert!(!command_markov_trigger(&size_50), "50 commands not enough");
            let size_100 = TerminalEvent::HistorySizeChanged { total_commands: 100 };
            assert!(command_markov_trigger(&size_100), "100 commands enough");
        }
    }

    #[test]
    fn command_markov_fires_on_any_command_after_bootstrap() {
        #[cfg(feature = "math-tools")]
        {
            let cmd = TerminalEvent::CommandExecuted {
                text: "ls".into(),
                exit_code: 0,
            };
            assert!(command_markov_trigger(&cmd), "should fire on any CommandExecuted");
        }
    }

    #[test]
    fn adinkra_project_type_selection() {
        #[cfg(feature = "griot-history")]
        {
            assert!(adinkra_trigger(&TerminalEvent::ProjectFileCreated {
                path: "Cargo.toml".into(),
                project_type: ProjectType::Rust,
            }));
            assert!(adinkra_trigger(&TerminalEvent::ProjectFileCreated {
                path: "package.json".into(),
                project_type: ProjectType::Node,
            }));
            assert!(!adinkra_trigger(&TerminalEvent::ProjectFileCreated {
                path: "Makefile".into(),
                project_type: ProjectType::Unknown,
            }));
        }
    }

    #[test]
    fn spectral_dashboard_pane_count_boundaries() {
        #[cfg(feature = "math-tools")]
        {
            let one = TerminalEvent::AgentPaneChanged { open_count: 1 };
            assert!(!spectral_dashboard_trigger(&one), "1 pane not enough");
            let two = TerminalEvent::AgentPaneChanged { open_count: 2 };
            assert!(spectral_dashboard_trigger(&two), "2 panes is enough");
            let five = TerminalEvent::AgentPaneChanged { open_count: 5 };
            assert!(spectral_dashboard_trigger(&five), "5 panes should fire");
        }
    }

    #[test]
    fn griot_history_threshold_boundary() {
        #[cfg(feature = "griot-history")]
        {
            let near = TerminalEvent::HistorySizeChanged { total_commands: 49 };
            assert!(!griot_history_trigger(&near), "49 below threshold");
            let at = TerminalEvent::HistorySizeChanged { total_commands: 50 };
            assert!(griot_history_trigger(&at), "50 is threshold");
            let above = TerminalEvent::HistorySizeChanged { total_commands: 51 };
            assert!(griot_history_trigger(&above), "51 above threshold");
        }
    }

    #[test]
    fn griot_history_pattern_threshold() {
        #[cfg(feature = "griot-history")]
        {
            let near = TerminalEvent::HistorySizeChanged { total_commands: 199 };
            assert!(!griot_history_pattern_trigger(&near), "199 below pattern threshold");
            let at = TerminalEvent::HistorySizeChanged { total_commands: 200 };
            assert!(griot_history_pattern_trigger(&at), "200 is pattern threshold");
            let above = TerminalEvent::HistorySizeChanged { total_commands: 1000 };
            assert!(griot_history_pattern_trigger(&above), "1000 above threshold");
        }
    }
}
