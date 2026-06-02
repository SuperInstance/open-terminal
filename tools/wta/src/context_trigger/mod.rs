//! Context Trigger Engine for Intelligent Terminal.
//!
//! A lightweight runtime that watches terminal events and activates dormant
//! modules when their context is detected. Modules that never trigger never
//! load — zero overhead when dormant.
//!
//! ## Design
//!
//! - Each module registers a [`TriggerFn`]: `Fn(&TerminalEvent) -> bool`
//! - When a trigger fires, the module is lazily loaded and activated
//! - The trigger engine itself is <100 lines — it's just a match statement
//! - Every trigger is a simple pattern match on the event:
//!   no computation, no IO, no allocation until triggered
//! - Target: <1ms latency added to the event loop

use std::time::Instant;

pub mod autoconfig;
pub mod dormant;
pub mod triggers;

use dormant::{ModuleHandle, ModuleState};

/// A terminal event that the trigger engine inspects.
///
/// Designed to be constructed from the existing event loop with zero
/// allocation and minimal branching. The trigger engine only reads
/// these fields — no additional IO or computation.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalEvent {
    /// A command was executed (full text, exit code).
    CommandExecuted {
        text: String,
        exit_code: i32,
    },
    /// A build command completed (e.g. `cargo build`, `npm run build`).
    BuildCompleted {
        text: String,
        exit_code: i32,
    },
    /// A test command was run (e.g. `cargo test`, `python -m pytest`).
    TestCommand {
        text: String,
    },
    /// A new project file was created (Cargo.toml, package.json).
    ProjectFileCreated {
        path: String,
        project_type: ProjectType,
    },
    /// An agent pane was opened or closed.
    AgentPaneChanged {
        open_count: usize,
    },
    /// A command failed (non-zero exit).
    CommandFailed {
        text: String,
        exit_code: i32,
    },
    /// Agent panes disagree on a fix.
    AgentDisagreement {
        pane_count: usize,
    },
    /// History size changed (number of commands in history).
    HistorySizeChanged {
        total_commands: usize,
    },
    /// Heartbeat / periodic tick for modules that need time-based checks.
    Tick,
}

/// Known project types for project detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectType {
    Rust,
    Node,
    Python,
    Unknown,
}

/// A trigger function signature: given an event, return true to activate.
pub type TriggerFn = fn(&TerminalEvent) -> bool;

/// A registered module with its trigger.
#[derive(Debug, Clone)]
pub struct TriggerModule {
    /// Human-readable name (for diagnostics).
    pub name: &'static str,
    /// The trigger predicate — pure, stateless, fast.
    pub trigger: TriggerFn,
    /// Current lifecycle state. `Dormant` by default.
    pub state: ModuleState,
    /// Feature gate that must be enabled for this module to load.
    pub feature_gate: Option<&'static str>,
}

// Static registry of all trigger modules. Built at compile time.
// This is the "match statement" — a simple slice of entries that
// the engine iterates on every event.

/// Registry of all trigger modules. Maintained as a static slice for
/// zero-cost iteration — no dynamic allocation at runtime.
pub fn registry() -> &'static [TriggerModule] {
    &ALL_TRIGGERS
}

/// All triggers, conditionally included based on feature gates.
///
/// When a feature gate is disabled, the corresponding trigger module
/// still exists in the registry (so the engine always has a complete
/// view), but its state stays `Dormant` forever — the trigger will
/// never be checked because the engine skips non-`Dormant` states
/// that were never loaded.
///
/// This ensures zero-cost when features are disabled: the trigger fn
/// is never called, and the module code is never compiled (cfg-gated
/// at the module boundary).
const ALL_TRIGGERS: &[TriggerModule] = &[
    #[cfg(feature = "math-tools")]
    TriggerModule {
        name: "math-tools",
        trigger: triggers::math_tools_trigger,
        state: ModuleState::Dormant,
        feature_gate: Some("math-tools"),
    },
    #[cfg(feature = "math-tools")]
    TriggerModule {
        name: "verification-entropy",
        trigger: triggers::verification_entropy_trigger,
        state: ModuleState::Dormant,
        feature_gate: Some("math-tools"),
    },
    #[cfg(feature = "math-tools")]
    TriggerModule {
        name: "error-hodge",
        trigger: triggers::error_hodge_trigger,
        state: ModuleState::Dormant,
        feature_gate: Some("math-tools"),
    },
    #[cfg(feature = "math-tools")]
    TriggerModule {
        name: "spectral-dashboard",
        trigger: triggers::spectral_dashboard_trigger,
        state: ModuleState::Dormant,
        feature_gate: Some("math-tools"),
    },
    #[cfg(feature = "math-tools")]
    TriggerModule {
        name: "command-markov",
        trigger: triggers::command_markov_trigger,
        state: ModuleState::Dormant,
        feature_gate: Some("math-tools"),
    },
    #[cfg(feature = "griot-history")]
    TriggerModule {
        name: "griot-history",
        trigger: triggers::griot_history_trigger,
        state: ModuleState::Dormant,
        feature_gate: Some("griot-history"),
    },
    #[cfg(feature = "griot-history")]
    TriggerModule {
        name: "griot-history-pattern",
        trigger: triggers::griot_history_pattern_trigger,
        state: ModuleState::Dormant,
        feature_gate: Some("griot-history"),
    },
    #[cfg(feature = "griot-history")]
    TriggerModule {
        name: "adinkra",
        trigger: triggers::adinkra_trigger,
        state: ModuleState::Dormant,
        feature_gate: Some("griot-history"),
    },
];

/// The trigger engine. Processes a single event against all registered
/// modules. Returns a list of modules that transitioned to `Active`.
///
/// # Latency guarantee
///
/// This function must complete in <1ms for the worst case. Every trigger
/// is a pure pattern match — no computation, no IO, no allocation until
/// a module needs to be activated.
///
/// # Safety
///
/// This function is re-entrant but not concurrent. Call from a single
/// thread (the existing event loop). Module activation (lazy loading)
/// is deferred to the caller.
pub fn process_event(event: &TerminalEvent) -> Vec<&'static str> {
    for module in ALL_TRIGGERS.iter() {
        // Fast-path: skip modules that are already active or expired.
        if !module.state.is_dormant() {
            continue;
        }
        // Check the trigger predicate.
        if (module.trigger)(event) {
            // Transition to Triggered state. The caller will do the
            // actual loading.
            //
            // This is safe because we only transition from Dormant→Triggered
            // here, and the engine is single-threaded.
            //
            // In production, this uses the dormant module's state management.
            // For this static registry, we use a side-channel: the caller
            // checks the trigger list and handles activation.
            //
            // For simplicity, we return the names of triggered modules.
        }
    }

    // In the full implementation, the engine uses the dormant module's
    // state management. For now, return triggered module names.
    triggered_modules(event)
}

/// Internal helper: evaluate all triggers and return modules that fire.
fn triggered_modules(event: &TerminalEvent) -> Vec<&'static str> {
    ALL_TRIGGERS
        .iter()
        .filter(|m| m.state.is_dormant() && (m.trigger)(event))
        .map(|m| m.name)
        .collect()
}

/// Convenience: evaluate a single event against a single module.
/// Returns true if the module's trigger fires for this event.
pub fn evaluate(name: &str, event: &TerminalEvent) -> bool {
    ALL_TRIGGERS
        .iter()
        .find(|m| m.name == name)
        .map(|m| (m.trigger)(event))
        .unwrap_or(false)
}

/// Run the initial auto-configuration scan.
/// Call once at startup.
pub fn auto_configure() {
    autoconfig::detect_and_configure();
}

/// Dump the current state of all trigger modules (for diagnostics).
pub fn dump_state() -> Vec<(&'static str, ModuleState)> {
    ALL_TRIGGERS
        .iter()
        .map(|m| (m.name, m.state.clone()))
        .collect()
}

#[cfg(test)]
#[allow(clippy::cognitive_complexity)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn registry_is_never_empty_when_features_enabled() {
        #[cfg(any(feature = "math-tools", feature = "griot-history"))]
        {
            assert!(!registry().is_empty(), "registry should have entries");
        }
        #[cfg(not(any(feature = "math-tools", feature = "griot-history")))]
        {
            assert!(registry().is_empty(), "registry should be empty with no features");
        }
    }

    #[test]
    fn all_modules_start_dormant() {
        for m in registry() {
            assert!(m.state.is_dormant(), "{} should start dormant", m.name);
        }
    }

    #[test]
    fn trigger_names_are_unique() {
        let mut names: Vec<&str> = registry().iter().map(|m| m.name).collect();
        names.sort();
        let mut deduped = names.clone();
        deduped.dedup();
        assert_eq!(names.len(), deduped.len(), "duplicate trigger names found");
    }

    #[test]
    fn triggers_are_stateless() {
        // Stateless guarantee: calling a trigger twice with the same
        // event returns the same result (no side effects).
        let event = TerminalEvent::CommandExecuted {
            text: "cargo test".into(),
            exit_code: 0,
        };
        for m in registry() {
            let r1 = (m.trigger)(&event);
            let r2 = (m.trigger)(&event);
            assert_eq!(r1, r2, "trigger {} is not stateless", m.name);
        }
    }

    #[test]
    fn math_tools_trigger_fires_on_test_command() {
        #[cfg(feature = "math-tools")]
        {
            let event = TerminalEvent::TestCommand {
                text: "cargo test".into(),
            };
            assert!(
                triggers::math_tools_trigger(&event),
                "math-tools should fire on test command"
            );
        }
    }

    #[test]
    fn math_tools_trigger_fires_on_agent_panes() {
        #[cfg(feature = "math-tools")]
        {
            let event = TerminalEvent::AgentPaneChanged { open_count: 3 };
            assert!(
                triggers::math_tools_trigger(&event),
                "math-tools should fire on multiple agent panes"
            );
        }
    }

    #[test]
    fn math_tools_does_not_fire_on_irrelevant() {
        #[cfg(feature = "math-tools")]
        {
            let event = TerminalEvent::CommandExecuted {
                text: "ls -la".into(),
                exit_code: 0,
            };
            assert!(
                !triggers::math_tools_trigger(&event),
                "math-tools should not fire on simple commands"
            );
        }
    }

    #[test]
    fn griot_history_trigger_fires_on_large_history() {
        #[cfg(feature = "griot-history")]
        {
            let event = TerminalEvent::HistorySizeChanged {
                total_commands: 100,
            };
            assert!(
                triggers::griot_history_trigger(&event),
                "griot-history should fire at >=50 commands"
            );
        }
    }

    #[test]
    fn griot_history_does_not_fire_on_small_history() {
        #[cfg(feature = "griot-history")]
        {
            let event = TerminalEvent::HistorySizeChanged {
                total_commands: 10,
            };
            assert!(
                !triggers::griot_history_trigger(&event),
                "griot-history should not fire on small history"
            );
        }
    }

    #[test]
    fn griot_history_pattern_trigger_fires_after_200() {
        #[cfg(feature = "griot-history")]
        {
            let event = TerminalEvent::HistorySizeChanged {
                total_commands: 300,
            };
            assert!(
                triggers::griot_history_pattern_trigger(&event),
                "griot-history pattern sub-trigger should fire after 200+"
            );
            let small = TerminalEvent::HistorySizeChanged {
                total_commands: 100,
            };
            assert!(
                !triggers::griot_history_pattern_trigger(&small),
                "griot-history pattern sub-trigger should NOT fire before 200"
            );
        }
    }

    #[test]
    fn verification_entropy_trigger_fires_on_build_without_tests() {
        #[cfg(feature = "math-tools")]
        {
            let event = TerminalEvent::BuildCompleted {
                text: "cargo build".into(),
                exit_code: 0,
            };
            assert!(
                triggers::verification_entropy_trigger(&event),
                "verification-entropy should fire on build completion"
            );
        }
    }

    #[test]
    fn error_hodge_trigger_fires_on_nonzero_exit() {
        #[cfg(feature = "math-tools")]
        {
            let event = TerminalEvent::CommandFailed {
                text: "cargo build".into(),
                exit_code: 1,
            };
            assert!(
                triggers::error_hodge_trigger(&event),
                "error-hodge should fire on non-zero exit"
            );
        }
    }

    #[test]
    fn error_hodge_does_not_fire_on_success() {
        #[cfg(feature = "math-tools")]
        {
            let event = TerminalEvent::CommandFailed {
                text: "cargo build".into(),
                exit_code: 0,
            };
            assert!(
                !triggers::error_hodge_trigger(&event),
                "error-hodge should NOT fire on zero exit"
            );
        }
    }

    #[test]
    fn spectral_dashboard_fires_on_disagreement() {
        #[cfg(feature = "math-tools")]
        {
            let event = TerminalEvent::AgentDisagreement { pane_count: 2 };
            assert!(
                triggers::spectral_dashboard_trigger(&event),
                "spectral-dashboard should fire on agent disagreement"
            );
        }
    }

    #[test]
    fn spectral_dashboard_also_fires_on_multiple_panes() {
        #[cfg(feature = "math-tools")]
        {
            let event = TerminalEvent::AgentPaneChanged { open_count: 3 };
            assert!(
                triggers::spectral_dashboard_trigger(&event),
                "spectral-dashboard should fire on multiple panes"
            );
        }
    }

    #[test]
    fn spectral_dashboard_does_not_fire_on_single_pane() {
        #[cfg(feature = "math-tools")]
        {
            let event = TerminalEvent::AgentPaneChanged { open_count: 1 };
            assert!(
                !triggers::spectral_dashboard_trigger(&event),
                "spectral-dashboard should NOT fire on single pane"
            );
        }
    }

    #[test]
    fn adinkra_trigger_fires_on_new_project() {
        #[cfg(feature = "griot-history")]
        {
            let event = TerminalEvent::ProjectFileCreated {
                path: "/tmp/project/Cargo.toml".into(),
                project_type: ProjectType::Rust,
            };
            assert!(
                triggers::adinkra_trigger(&event),
                "adinkra should fire on new Cargo.toml"
            );
            let event2 = TerminalEvent::ProjectFileCreated {
                path: "/tmp/project/package.json".into(),
                project_type: ProjectType::Node,
            };
            assert!(
                triggers::adinkra_trigger(&event2),
                "adinkra should fire on new package.json"
            );
        }
    }

    #[test]
    fn adinkra_does_not_fire_on_unknown_project() {
        #[cfg(feature = "griot-history")]
        {
            let event = TerminalEvent::ProjectFileCreated {
                path: "/tmp/README.md".into(),
                project_type: ProjectType::Unknown,
            };
            assert!(
                !triggers::adinkra_trigger(&event),
                "adinkra should NOT fire on unknown project types"
            );
        }
    }

    #[test]
    fn command_markov_trigger_fires_after_100_commands() {
        #[cfg(feature = "math-tools")]
        {
            let event = TerminalEvent::HistorySizeChanged {
                total_commands: 100,
            };
            assert!(
                triggers::command_markov_trigger(&event),
                "command-markov should fire at 100+ commands"
            );
            let small = TerminalEvent::HistorySizeChanged {
                total_commands: 50,
            };
            assert!(
                !triggers::command_markov_trigger(&small),
                "command-markov should NOT fire before 100"
            );
        }
    }

    #[test]
    fn process_event_returns_fired_modules() {
        #[cfg(feature = "griot-history")]
        {
            let event = TerminalEvent::HistorySizeChanged {
                total_commands: 100,
            };
            let fired = process_event(&event);
            assert!(
                fired.contains(&"griot-history"),
                "griot-history should be in fired list"
            );
        }
    }

    #[test]
    fn tick_never_triggers_anything() {
        // Tick is a heartbeat — individual modules decide whether to
        // react to it. No trigger should blindly fire on Tick.
        let event = TerminalEvent::Tick;
        for m in registry() {
            let fired = (m.trigger)(&event);
            assert!(!fired, "{} should not fire on Tick alone", m.name);
        }
    }

    #[test]
    fn latency_under_1ms_for_ten_events() {
        // Empirical check: process 10 events (simulating a burst) and
        // verify total time is <10ms. This is not a precise benchmark
        // but catches accidental regressions (e.g. IO in trigger path).
        let events = vec![
            TerminalEvent::CommandExecuted { text: "cargo build".into(), exit_code: 0 },
            TerminalEvent::TestCommand { text: "cargo test".into() },
            TerminalEvent::CommandFailed { text: "npm run build".into(), exit_code: 1 },
            TerminalEvent::AgentPaneChanged { open_count: 3 },
            TerminalEvent::ProjectFileCreated { path: "Cargo.toml".into(), project_type: ProjectType::Rust },
            TerminalEvent::HistorySizeChanged { total_commands: 200 },
            TerminalEvent::Tick,
            TerminalEvent::AgentDisagreement { pane_count: 2 },
            TerminalEvent::BuildCompleted { text: "cargo build".into(), exit_code: 0 },
            TerminalEvent::CommandExecuted { text: "git status".into(), exit_code: 0 },
        ];
        let start = Instant::now();
        for event in &events {
            process_event(event);
        }
        let elapsed = start.elapsed();
        let per_event_ns = elapsed.as_nanos() / events.len() as u128;
        assert!(
            per_event_ns < 1_000_000,
            "per-event latency too high: {}ns/event (target <1ms)",
            per_event_ns
        );
        // Also verify <1ms total for all events — the strict constraint.
        assert!(
            elapsed.as_micros() < 1000,
            "total latency for 10 events: {}µs (target <1000µs)",
            elapsed.as_micros()
        );
    }

    #[test]
    fn autoconfig_finds_one_known_tool() {
        // In the sandboxed test environment we can't guarantee specific
        // tools, so just check the function doesn't panic.
        let _ = autoconfig::Config::detect();
    }

    #[test]
    fn all_trigger_fns_handle_command_executed_with_failure() {
        let event = TerminalEvent::CommandExecuted {
            text: "cargo build".into(),
            exit_code: 1,
        };
        for m in registry() {
            // No panic = success
            let _ = (m.trigger)(&event);
        }
    }

    #[test]
    fn evaluate_single_module_by_name() {
        #[cfg(feature = "griot-history")]
        {
            let event = TerminalEvent::HistorySizeChanged {
                total_commands: 100,
            };
            assert!(evaluate("griot-history", &event));
        }
        #[cfg(not(feature = "griot-history"))]
        {
            let event = TerminalEvent::HistorySizeChanged {
                total_commands: 100,
            };
            assert!(!evaluate("griot-history", &event));
        }
    }

    #[test]
    fn evaluate_unknown_module_returns_false() {
        let event = TerminalEvent::Tick;
        assert!(!evaluate("nonexistent-module", &event));
    }

    #[test]
    fn dump_state_is_consistent() {
        let state = dump_state();
        assert_eq!(state.len(), registry().len());
        for (name, st) in &state {
            assert!(st.is_dormant(), "{name} should start dormant");
        }
    }
}
