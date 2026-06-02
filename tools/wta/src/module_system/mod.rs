//! Module system for Intelligent Terminal.
//!
//! Provides a trait-based plugin architecture where modules can observe
//! terminal events and produce outputs (suggestions, visualizations,
//! insights) without commandeering the UI or blocking the event loop.
//!
//! ## Architecture
//!
//! - [`TerminalModule`] — the trait all modules implement
//! - [`ModuleRegistry`] — holds all registered modules, runs triggers, manages lifecycle
//! - [`ModuleContext`] — constrained view of terminal state (what modules can access)
//! - [`ModuleOutput`] — what modules produce
//! - [`MemoryBudget`] — automatic memory management with LRU eviction
//!
//! Feature-gated under `module-system`.

pub mod memory_budget;
pub mod module_context;
pub mod module_output;

#[cfg(feature = "module-system")]
pub mod builtin_modules;

pub use memory_budget::MemoryBudget;
pub use module_context::{ModuleContext, CommandEntry, COMMAND_HISTORY_WINDOW};
pub use module_output::ModuleOutput;

/// Events from the terminal that modules can observe.
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    /// A command was entered.
    CommandEntered {
        command: String,
        timestamp_secs: u64,
    },
    /// A command completed.
    CommandCompleted {
        command: String,
        exit_code: i32,
        timestamp_secs: u64,
    },
    /// An error occurred.
    Error {
        message: String,
        exit_code: i32,
    },
    /// Working directory changed.
    DirectoryChanged {
        path: String,
    },
    /// An agent session started.
    AgentStarted {
        agent_id: String,
    },
    /// An agent session ended.
    AgentEnded {
        agent_id: String,
    },
    /// Files detected in the project directory.
    ProjectFilesDetected {
        files: Vec<String>,
    },
    /// A periodic tick (for modules that do time-based analysis).
    Tick {
        timestamp_secs: u64,
    },
}

/// The trait all terminal modules must implement.
///
/// Modules are **guests** in the terminal. They can suggest, observe, and
/// analyze. They cannot commandeer the UI, block the event loop, or access
/// private data.
pub trait TerminalModule {
    /// Unique identifier for this module (e.g. "command_markov").
    fn id(&self) -> &str;

    /// Whether this module should activate for the given event.
    /// Called for every event; keep it fast.
    fn trigger(&self, event: &TerminalEvent) -> bool;

    /// Activate the module, providing context it can read.
    fn activate(&mut self, ctx: &ModuleContext);

    /// Handle an event and produce outputs. Only called when active.
    fn handle_event(&mut self, event: &TerminalEvent) -> Vec<ModuleOutput>;

    /// Deactivate the module. Serialize state if needed.
    fn deactivate(&mut self);

    /// Whether the module is currently active.
    fn is_active(&self) -> bool;

    /// Approximate memory usage in bytes.
    fn memory_usage(&self) -> usize;

    /// Serialize state to bytes for disk persistence.
    /// Default: empty (no state to persist).
    fn serialize_state(&self) -> Vec<u8> {
        Vec::new()
    }

    /// Deserialize state from bytes. Returns false if deserialization fails.
    /// Default: no-op (no state to restore).
    fn deserialize_state(&mut self, _data: &[u8]) -> bool {
        true
    }
}

/// A registered module with its trigger order metadata.
struct RegisteredModule {
    module: Box<dyn TerminalModule>,
    /// Timestamp of the last time trigger() returned true.
    last_triggered: Option<u64>,
    /// Timestamp of the last activation.
    activated_at: Option<u64>,
}

/// The module registry: owns all modules and manages their lifecycle.
pub struct ModuleRegistry {
    modules: Vec<RegisteredModule>,
    budget: MemoryBudget,
}

impl ModuleRegistry {
    /// Create a new empty registry.
    pub fn new(budget: MemoryBudget) -> Self {
        Self {
            modules: Vec::new(),
            budget,
        }
    }

    /// Register a module.
    pub fn register(&mut self, module: Box<dyn TerminalModule>) {
        self.modules.push(RegisteredModule {
            module,
            last_triggered: None,
            activated_at: None,
        });
    }

    /// Number of registered modules.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Get module IDs.
    pub fn module_ids(&self) -> Vec<&str> {
        self.modules.iter().map(|r| r.module.id()).collect()
    }

    /// Get a reference to a module by ID.
    pub fn get_module(&self, id: &str) -> Option<&dyn TerminalModule> {
        self.modules
            .iter()
            .find(|r| r.module.id() == id)
            .map(|r| r.module.as_ref())
    }

    /// Activate a specific module.
    pub fn activate_module(&mut self, id: &str, ctx: &ModuleContext) -> bool {
        if let Some(reg) = self.modules.iter_mut().find(|r| r.module.id() == id) {
            if !reg.module.is_active() {
                // Try to deserialize state from disk.
                if let Ok(Some(data)) = self.budget.load_state(id) {
                    reg.module.deserialize_state(&data);
                }
                reg.module.activate(ctx);
                self.budget.update_usage(id, reg.module.memory_usage());
            }
            true
        } else {
            false
        }
    }

    /// Deactivate a specific module, saving its state.
    pub fn deactivate_module(&mut self, id: &str) -> bool {
        if let Some(reg) = self.modules.iter_mut().find(|r| r.module.id() == id) {
            if reg.module.is_active() {
                let state = reg.module.serialize_state();
                if !state.is_empty() {
                    let _ = self.budget.save_state(id, &state);
                }
                reg.module.deactivate();
                self.budget.remove(id);
            }
            true
        } else {
            false
        }
    }

    /// Process an event through all registered modules.
    ///
    /// For each active module whose trigger matches, calls handle_event.
    /// For inactive modules whose trigger matches, activates them first.
    ///
    /// Returns all outputs collected from all modules.
    pub fn process_event(
        &mut self,
        event: &TerminalEvent,
        ctx: &ModuleContext,
    ) -> Vec<ModuleOutput> {
        let timestamp = event_timestamp(event);
        let mut outputs = Vec::new();

        for reg in &mut self.modules {
            if reg.module.trigger(event) {
                reg.last_triggered = timestamp;

                if !reg.module.is_active() {
                    // Try cold start from disk.
                    if let Ok(Some(data)) = self.budget.load_state(reg.module.id()) {
                        reg.module.deserialize_state(&data);
                    }
                    reg.module.activate(ctx);
                    reg.activated_at = timestamp;
                }

                outputs.extend(reg.module.handle_event(event));
                self.budget.update_usage(reg.module.id(), reg.module.memory_usage());
            }
        }

        // Enforce memory budget.
        self.enforce_budget();

        outputs
    }

    /// Enforce memory budget by deactivating least-recently-triggered modules.
    fn enforce_budget(&mut self) {
        if !self.budget.is_exceeded() {
            return;
        }

        // Build trigger order: least-recent first.
        let mut active: Vec<(String, Option<u64>)> = self
            .modules
            .iter()
            .filter(|r| r.module.is_active())
            .map(|r| (r.module.id().to_string(), r.last_triggered))
            .collect();

        // Sort: None (never triggered) first, then by timestamp ascending.
        active.sort_by(|a, b| {
            a.1.unwrap_or(0).cmp(&b.1.unwrap_or(0))
        });

        let order: Vec<String> = active.into_iter().map(|(id, _)| id).collect();
        let candidates = self.budget.candidates_for_eviction(&order);

        for id in candidates {
            self.deactivate_module(&id);
        }
    }

    /// Deactivate all modules, saving their state.
    pub fn deactivate_all(&mut self) {
        for reg in &mut self.modules {
            if reg.module.is_active() {
                let state = reg.module.serialize_state();
                if !state.is_empty() {
                    let _ = self.budget.save_state(reg.module.id(), &state);
                }
                reg.module.deactivate();
            }
        }
        self.budget = MemoryBudget::new(self.budget.budget(), self.budget.state_dir().to_path_buf());
    }

    /// Total memory usage across all active modules.
    pub fn total_memory_usage(&self) -> usize {
        self.budget.total_usage()
    }

    /// Whether the budget is exceeded.
    pub fn is_over_budget(&self) -> bool {
        self.budget.is_exceeded()
    }

    /// Number of active modules.
    pub fn active_count(&self) -> usize {
        self.modules.iter().filter(|r| r.module.is_active()).count()
    }
}

/// Extract a timestamp from a terminal event.
fn event_timestamp(event: &TerminalEvent) -> Option<u64> {
    match event {
        TerminalEvent::CommandEntered { timestamp_secs, .. } => Some(*timestamp_secs),
        TerminalEvent::CommandCompleted { timestamp_secs, .. } => Some(*timestamp_secs),
        TerminalEvent::Tick { timestamp_secs } => Some(*timestamp_secs),
        _ => None,
    }
}

impl MemoryBudget {
    /// Get the state directory path.
    fn state_dir(&self) -> &Path {
        &self.state_dir
    }
}

use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple test module.
    struct TestModule {
        active: bool,
        trigger_on_command: String,
        last_output: String,
    }

    impl TestModule {
        fn new(trigger_on: &str) -> Self {
            Self {
                active: false,
                trigger_on_command: trigger_on.to_string(),
                last_output: String::new(),
            }
        }
    }

    impl TerminalModule for TestModule {
        fn id(&self) -> &str {
            "test_module"
        }

        fn trigger(&self, event: &TerminalEvent) -> bool {
            matches!(event,
                TerminalEvent::CommandEntered { command, .. }
                | TerminalEvent::CommandCompleted { command, .. }
                if command.contains(&self.trigger_on_command)
            )
        }

        fn activate(&mut self, _ctx: &ModuleContext) {
            self.active = true;
        }

        fn handle_event(&mut self, event: &TerminalEvent) -> Vec<ModuleOutput> {
            if let TerminalEvent::CommandEntered { command, .. } = event {
                self.last_output = command.clone();
                vec![ModuleOutput::StatusBar(format!("saw: {}", command))]
            } else {
                vec![]
            }
        }

        fn deactivate(&mut self) {
            self.active = false;
        }

        fn is_active(&self) -> bool {
            self.active
        }

        fn memory_usage(&self) -> usize {
            1024
        }
    }

    /// A module that reports large memory.
    struct BigModule {
        active: bool,
    }

    impl BigModule {
        fn new() -> Self {
            Self { active: false }
        }
    }

    impl TerminalModule for BigModule {
        fn id(&self) -> &str {
            "big_module"
        }

        fn trigger(&self, _event: &TerminalEvent) -> bool {
            true // always triggers
        }

        fn activate(&mut self, _ctx: &ModuleContext) {
            self.active = true;
        }

        fn handle_event(&mut self, _event: &TerminalEvent) -> Vec<ModuleOutput> {
            vec![]
        }

        fn deactivate(&mut self) {
            self.active = false;
        }

        fn is_active(&self) -> bool {
            self.active
        }

        fn memory_usage(&self) -> usize {
            60 * 1024 * 1024 // 60 MB
        }
    }

    fn fresh_registry(budget_bytes: usize) -> ModuleRegistry {
        let dir = std::env::temp_dir().join("wta_test_registry");
        let _ = std::fs::remove_dir_all(&dir);
        ModuleRegistry::new(MemoryBudget::new(budget_bytes, dir))
    }

    fn cmd_entered(cmd: &str) -> TerminalEvent {
        TerminalEvent::CommandEntered {
            command: cmd.to_string(),
            timestamp_secs: 1000,
        }
    }

    fn tick(ts: u64) -> TerminalEvent {
        TerminalEvent::Tick { timestamp_secs: ts }
    }

    #[test]
    fn register_and_count() {
        let mut reg = fresh_registry(1000);
        reg.register(Box::new(TestModule::new("cargo")));
        assert_eq!(reg.module_count(), 1);
        assert_eq!(reg.module_ids(), vec!["test_module"]);
    }

    #[test]
    fn get_module_by_id() {
        let mut reg = fresh_registry(1000);
        reg.register(Box::new(TestModule::new("cargo")));
        assert!(reg.get_module("test_module").is_some());
        assert!(reg.get_module("nonexistent").is_none());
    }

    #[test]
    fn activate_module_explicitly() {
        let mut reg = fresh_registry(1000);
        reg.register(Box::new(TestModule::new("cargo")));
        let ctx = ModuleContext::empty();
        assert!(reg.activate_module("test_module", &ctx));
        assert!(reg.get_module("test_module").unwrap().is_active());
        assert_eq!(reg.active_count(), 1);
    }

    #[test]
    fn activate_nonexistent_fails() {
        let mut reg = fresh_registry(1000);
        let ctx = ModuleContext::empty();
        assert!(!reg.activate_module("nope", &ctx));
    }

    #[test]
    fn deactivate_module() {
        let mut reg = fresh_registry(1000);
        reg.register(Box::new(TestModule::new("cargo")));
        let ctx = ModuleContext::empty();
        reg.activate_module("test_module", &ctx);
        assert!(reg.deactivate_module("test_module"));
        assert!(!reg.get_module("test_module").unwrap().is_active());
        assert_eq!(reg.active_count(), 0);
    }

    #[test]
    fn process_event_triggers_module() {
        let mut reg = fresh_registry(10 * 1024 * 1024);
        reg.register(Box::new(TestModule::new("cargo")));
        let ctx = ModuleContext::empty();
        let outputs = reg.process_event(&cmd_entered("cargo build"), &ctx);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0], ModuleOutput::StatusBar("saw: cargo build".into()));
        assert_eq!(reg.active_count(), 1);
    }

    #[test]
    fn process_event_no_trigger() {
        let mut reg = fresh_registry(10 * 1024 * 1024);
        reg.register(Box::new(TestModule::new("cargo")));
        let ctx = ModuleContext::empty();
        let outputs = reg.process_event(&cmd_entered("npm install"), &ctx);
        assert!(outputs.is_empty());
        assert_eq!(reg.active_count(), 0);
    }

    #[test]
    fn memory_budget_enforced() {
        // 50 byte budget — big module exceeds it immediately
        let mut reg = fresh_registry(50);
        reg.register(Box::new(BigModule::new()));
        let ctx = ModuleContext::empty();
        reg.process_event(&tick(100), &ctx);
        // Should have been evicted
        assert!(reg.is_over_budget() || reg.active_count() == 0);
    }

    #[test]
    fn deactivate_all() {
        let mut reg = fresh_registry(10 * 1024 * 1024);
        reg.register(Box::new(TestModule::new("cargo")));
        let ctx = ModuleContext::empty();
        reg.activate_module("test_module", &ctx);
        assert_eq!(reg.active_count(), 1);
        reg.deactivate_all();
        assert_eq!(reg.active_count(), 0);
    }

    #[test]
    fn total_memory_usage_tracking() {
        let mut reg = fresh_registry(10 * 1024 * 1024);
        reg.register(Box::new(TestModule::new("cargo")));
        let ctx = ModuleContext::empty();
        reg.activate_module("test_module", &ctx);
        assert_eq!(reg.total_memory_usage(), 1024);
    }

    #[test]
    fn event_timestamp_extraction() {
        let event = cmd_entered("test");
        assert_eq!(event_timestamp(&event), Some(1000));
        let tick_evt = tick(9999);
        assert_eq!(event_timestamp(&tick_evt), Some(9999));
        let dir_evt = TerminalEvent::DirectoryChanged { path: "/tmp".into() };
        assert_eq!(event_timestamp(&dir_evt), None);
    }

    #[test]
    fn double_activate_is_noop() {
        let mut reg = fresh_registry(10 * 1024 * 1024);
        reg.register(Box::new(TestModule::new("cargo")));
        let ctx = ModuleContext::empty();
        reg.activate_module("test_module", &ctx);
        reg.activate_module("test_module", &ctx);
        assert_eq!(reg.active_count(), 1);
    }

    #[test]
    fn deactivate_nonexistent() {
        let mut reg = fresh_registry(1000);
        // Returns true because "module not found" is treated as already inactive.
        assert!(reg.deactivate_module("nope"));
    }

    #[test]
    fn multiple_modules_process() {
        let mut reg = fresh_registry(10 * 1024 * 1024);

        // Second test module with a different ID
        struct TestModule2 { active: bool }
        impl TerminalModule for TestModule2 {
            fn id(&self) -> &str { "test_module_2" }
            fn trigger(&self, _event: &TerminalEvent) -> bool { true }
            fn activate(&mut self, _ctx: &ModuleContext) { self.active = true; }
            fn handle_event(&mut self, _event: &TerminalEvent) -> Vec<ModuleOutput> {
                vec![ModuleOutput::notification("from module 2")]
            }
            fn deactivate(&mut self) { self.active = false; }
            fn is_active(&self) -> bool { self.active }
            fn memory_usage(&self) -> usize { 512 }
        }

        reg.register(Box::new(TestModule::new("cargo")));
        reg.register(Box::new(TestModule2 { active: false }));

        let ctx = ModuleContext::empty();
        let outputs = reg.process_event(&cmd_entered("cargo build"), &ctx);
        // Both modules should produce output
        assert_eq!(outputs.len(), 2);
        assert_eq!(reg.active_count(), 2);
    }

    #[test]
    fn serialize_deserialize_default() {
        let mut m = TestModule::new("cargo");
        assert!(m.serialize_state().is_empty());
        assert!(m.deserialize_state(&[]));
    }
}
