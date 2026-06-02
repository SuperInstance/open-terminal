//! Module lifecycle states for the trigger engine.
//!
//! Each module transitions through four states:
//!
//! 1. `Dormant` — Not loaded, zero memory. The module's code is not
//!    compiled (cfg-gated at the module boundary). The trigger predicate
//!    exists but is never evaluated.
//!
//! 2. `Triggered` — The trigger predicate matched. Loading in progress.
//!    The engine has identified that this module's context is present.
//!    The module's code is being lazily loaded (first access).
//!
//! 3. `Active` — Running, consuming events. The module is fully loaded
//!    and processing events. It now receives every relevant event.
//!
//! 4. `Expired` — No longer relevant, unload. The module has determined
//!    its context is gone (e.g., the user closed all agent panes).
//!    Resources are released, state transitions back to Dormant.
//!
//! ## Design
//!
//! Each state transition is a single function call. No threads, no
//! background processes. The trigger runs in the existing event loop.
//!
//! ## Thread safety
//!
//! `ModuleState` implements `Clone` and `Send` for diagnostic
//! serialization but is NOT `Sync` — the trigger engine is single-
//! threaded by design. Use `Rc<RefCell<ModuleState>>` if sharing
//! across the engine and module code within the same thread.

use std::fmt;

/// The lifecycle state of a trigger module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleState {
    /// Not loaded, zero memory. Module code is not compiled.
    Dormant,
    /// Trigger fired, loading in progress.
    Triggered,
    /// Running, consuming events.
    Active,
    /// No longer relevant, ready to unload.
    Expired,
}

impl ModuleState {
    /// Returns true if the module is dormant (not loaded).
    pub fn is_dormant(&self) -> bool {
        matches!(self, ModuleState::Dormant)
    }

    /// Returns true if the module is active (loaded and processing).
    pub fn is_active(&self) -> bool {
        matches!(self, ModuleState::Active)
    }

    /// Returns true if the module has been triggered (loading in progress).
    pub fn is_triggered(&self) -> bool {
        matches!(self, ModuleState::Triggered)
    }

    /// Returns true if the module is expired (ready to unload).
    pub fn is_expired(&self) -> bool {
        matches!(self, ModuleState::Expired)
    }

    /// Transition to the next state, returning true if the transition
    /// was valid, false otherwise.
    ///
    /// Valid transitions:
    /// - Dormant → Triggered (trigger fired)
    /// - Triggered → Active (loading complete)
    /// - Active → Expired (context lost)
    /// - Expired → Dormant (fully unloaded)
    /// - Dormant → Active (direct, for auto-configured modules)
    pub fn transition_to(&mut self, target: ModuleState) -> bool {
        use ModuleState::*;
        match (&self, &target) {
            (Dormant, Triggered) | (Dormant, Active) => {
                *self = target;
                true
            }
            (Triggered, Active) => {
                *self = target;
                true
            }
            (Active, Expired) => {
                *self = target;
                true
            }
            (Expired, Dormant) => {
                *self = target;
                true
            }
            _ => {
                // Invalid transition — stay in current state.
                false
            }
        }
    }

    /// Force-set state without validation (for initial setup).
    pub fn set(&mut self, state: ModuleState) {
        *self = state;
    }
}

impl fmt::Display for ModuleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModuleState::Dormant => write!(f, "dormant"),
            ModuleState::Triggered => write!(f, "triggered"),
            ModuleState::Active => write!(f, "active"),
            ModuleState::Expired => write!(f, "expired"),
        }
    }
}

/// A handle to a module's state within the trigger engine.
///
/// This is a lightweight reference that the module code can use to
/// update its own state. In the current single-threaded design,
/// it wraps a `*mut ModuleState` for zero-cost access.
#[derive(Debug, Clone)]
pub struct ModuleHandle {
    /// Name of the module this handle belongs to.
    pub name: &'static str,
    /// Human-readable description of what this module does.
    pub description: &'static str,
    /// Current state (stored in the module's state cell).
    pub state: ModuleState,
}

impl ModuleHandle {
    /// Create a new module handle. Starts in `Dormant` state.
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        ModuleHandle {
            name,
            description,
            state: ModuleState::Dormant,
        }
    }

    /// Mark this module as triggered (trigger predicate matched).
    pub fn trigger(&mut self) {
        self.state.transition_to(ModuleState::Triggered);
    }

    /// Mark this module as active (loading complete).
    pub fn activate(&mut self) {
        self.state.transition_to(ModuleState::Active);
    }

    /// Mark this module as expired (context lost).
    pub fn expire(&mut self) {
        self.state.transition_to(ModuleState::Expired);
    }

    /// Reset to dormant (fully unloaded).
    pub fn reset(&mut self) {
        self.state.set(ModuleState::Dormant);
    }

    /// Returns true if this handle can receive events.
    pub fn is_listening(&self) -> bool {
        matches!(self.state, ModuleState::Active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dormant_starts_false_for_all_predicates() {
        let s = ModuleState::Dormant;
        assert!(s.is_dormant());
        assert!(!s.is_active());
        assert!(!s.is_triggered());
        assert!(!s.is_expired());
    }

    #[test]
    fn valid_transitions_succeed() {
        let mut s = ModuleState::Dormant;

        assert!(s.transition_to(ModuleState::Triggered));
        assert_eq!(s, ModuleState::Triggered);

        assert!(s.transition_to(ModuleState::Active));
        assert_eq!(s, ModuleState::Active);

        assert!(s.transition_to(ModuleState::Expired));
        assert_eq!(s, ModuleState::Expired);

        assert!(s.transition_to(ModuleState::Dormant));
        assert_eq!(s, ModuleState::Dormant);
    }

    #[test]
    fn direct_dormant_to_active_valid() {
        let mut s = ModuleState::Dormant;
        assert!(s.transition_to(ModuleState::Active));
        assert_eq!(s, ModuleState::Active);
    }

    #[test]
    fn invalid_transitions_rejected() {
        // Triggered → Dormant (invalid: must go Active first)
        let mut s = ModuleState::Triggered;
        assert!(!s.transition_to(ModuleState::Dormant));
        assert_eq!(s, ModuleState::Triggered);

        // Triggered → Expired (invalid: must go Active first)
        let mut s = ModuleState::Triggered;
        assert!(!s.transition_to(ModuleState::Expired));
        assert_eq!(s, ModuleState::Triggered);

        // Active → Dormant (invalid: must go Expired first)
        let mut s = ModuleState::Active;
        assert!(!s.transition_to(ModuleState::Dormant));
        assert_eq!(s, ModuleState::Active);

        // Expired → Triggered (invalid: must go Dormant first)
        let mut s = ModuleState::Expired;
        assert!(!s.transition_to(ModuleState::Triggered));
        assert_eq!(s, ModuleState::Expired);

        // Expired → Active (invalid)
        let mut s = ModuleState::Expired;
        assert!(!s.transition_to(ModuleState::Active));
        assert_eq!(s, ModuleState::Expired);
    }

    #[test]
    fn active_state_affects_listening() {
        let mut handle = ModuleHandle::new("test-module", "A test module");

        // Dormant: not listening
        assert!(!handle.is_listening());

        // Active: listening
        handle.activate();
        assert!(handle.is_listening());

        // Expired: not listening
        handle.expire();
        assert!(!handle.is_listening());
    }

    #[test]
    fn lifecycle_round_trip() {
        let mut handle = ModuleHandle::new("roundtrip", "Round-trip test");

        assert_eq!(handle.state, ModuleState::Dormant);

        handle.trigger();
        assert_eq!(handle.state, ModuleState::Triggered);

        handle.activate();
        assert_eq!(handle.state, ModuleState::Active);

        handle.expire();
        assert_eq!(handle.state, ModuleState::Expired);

        handle.reset();
        assert_eq!(handle.state, ModuleState::Dormant);
    }

    #[test]
    fn display_trait() {
        assert_eq!(format!("{}", ModuleState::Dormant), "dormant");
        assert_eq!(format!("{}", ModuleState::Triggered), "triggered");
        assert_eq!(format!("{}", ModuleState::Active), "active");
        assert_eq!(format!("{}", ModuleState::Expired), "expired");
    }

    #[test]
    fn set_skips_validation() {
        let mut s = ModuleState::Active;
        s.set(ModuleState::Dormant);
        // This is normally an invalid transition, but set() allows it.
        assert_eq!(s, ModuleState::Dormant);
    }
}
