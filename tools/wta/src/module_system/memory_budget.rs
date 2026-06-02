//! Memory budget management for modules.
//!
//! Each module reports its memory usage. The registry enforces a total budget
//! (default 50 MB). When exceeded, least-recently-triggered modules are
//! deactivated. Deactivated modules serialize state to disk; reactivation
//! deserializes it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Default total memory budget for all modules combined (bytes).
pub const DEFAULT_MEMORY_BUDGET: usize = 50 * 1024 * 1024; // 50 MB

/// Directory under which module state is serialized.
const MODULE_STATE_DIR: &str = "module_state";

/// Tracks memory usage per module and enforces the budget.
#[derive(Debug)]
pub struct MemoryBudget {
    /// Total budget in bytes.
    budget: usize,
    /// Current usage: module_id → bytes.
    usage: Vec<(String, usize)>,
    /// Base directory for serialized state.
    pub state_dir: PathBuf,
}
impl MemoryBudget {
    /// Create a new budget tracker.
    pub fn new(budget: usize, state_dir: PathBuf) -> Self {
        Self {
            budget,
            usage: Vec::new(),
            state_dir,
        }
    }

    /// Create with default budget.
    pub fn default_budget(state_dir: PathBuf) -> Self {
        Self::new(DEFAULT_MEMORY_BUDGET, state_dir)
    }

    /// Total budget in bytes.
    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Current total usage across all modules.
    pub fn total_usage(&self) -> usize {
        self.usage.iter().map(|(_, b)| *b).sum()
    }

    /// Remaining budget.
    pub fn remaining(&self) -> usize {
        self.budget.saturating_sub(self.total_usage())
    }

    /// Whether the budget is exceeded.
    pub fn is_exceeded(&self) -> bool {
        self.total_usage() > self.budget
    }

    /// Update a module's reported memory usage.
    pub fn update_usage(&mut self, module_id: &str, bytes: usize) {
        if let Some(entry) = self.usage.iter_mut().find(|(id, _)| id == module_id) {
            entry.1 = bytes;
        } else {
            self.usage.push((module_id.to_string(), bytes));
        }
    }

    /// Remove a module from tracking.
    pub fn remove(&mut self, module_id: &str) {
        self.usage.retain(|(id, _)| id != module_id);
    }

    /// Get usage for a specific module.
    pub fn usage_for(&self, module_id: &str) -> usize {
        self.usage
            .iter()
            .find(|(id, _)| id == module_id)
            .map(|(_, b)| *b)
            .unwrap_or(0)
    }

    /// Find modules that should be deactivated to bring usage under budget.
    ///
    /// Returns module IDs ordered from least-recently-triggered to most.
    /// The caller provides the trigger order (most recent last).
    pub fn candidates_for_eviction(
        &self,
        trigger_order: &[String], // ordered least-recent → most-recent
    ) -> Vec<String> {
        if !self.is_exceeded() {
            return Vec::new();
        }

        let mut excess = self.total_usage().saturating_sub(self.budget);
        let mut candidates = Vec::new();

        // Evict from least-recently-triggered (front of the list).
        for module_id in trigger_order {
            if excess == 0 {
                break;
            }
            let usage = self.usage_for(module_id);
            if usage > 0 {
                excess = excess.saturating_sub(usage);
                candidates.push(module_id.clone());
            }
        }

        candidates
    }

    /// Path for serialized state of a module.
    pub fn state_path(&self, module_id: &str) -> PathBuf {
        // Sanitize module_id for use as a filename.
        let safe_name: String = module_id
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        self.state_dir.join(MODULE_STATE_DIR).join(format!("{}.state", safe_name))
    }

    /// Serialize state bytes to disk.
    pub fn save_state(&self, module_id: &str, data: &[u8]) -> io::Result<()> {
        let path = self.state_path(module_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, data)
    }

    /// Deserialize state bytes from disk. Returns None if no state exists.
    pub fn load_state(&self, module_id: &str) -> io::Result<Option<Vec<u8>>> {
        let path = self.state_path(module_id);
        if path.exists() {
            Ok(Some(fs::read(&path)?))
        } else {
            Ok(None)
        }
    }

    /// Remove serialized state for a module.
    pub fn clear_state(&self, module_id: &str) -> io::Result<()> {
        let path = self.state_path(module_id);
        if path.exists() {
            fs::remove_file(&path)
        } else {
            Ok(())
        }
    }

    /// Number of tracked modules.
    pub fn tracked_count(&self) -> usize {
        self.usage.len()
    }
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self::default_budget(std::env::temp_dir().join("wta_modules"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_budget_is_50mb() {
        assert_eq!(DEFAULT_MEMORY_BUDGET, 50 * 1024 * 1024);
    }

    #[test]
    fn new_budget_custom() {
        let mb = MemoryBudget::new(1000, PathBuf::from("/tmp/test_modules"));
        assert_eq!(mb.budget(), 1000);
        assert_eq!(mb.total_usage(), 0);
        assert_eq!(mb.remaining(), 1000);
        assert!(!mb.is_exceeded());
    }

    #[test]
    fn update_usage() {
        let mut mb = MemoryBudget::new(1000, PathBuf::from("/tmp/test"));
        mb.update_usage("mod_a", 400);
        mb.update_usage("mod_b", 300);
        assert_eq!(mb.total_usage(), 700);
        assert_eq!(mb.remaining(), 300);
        assert_eq!(mb.usage_for("mod_a"), 400);
        assert_eq!(mb.usage_for("mod_b"), 300);
        assert_eq!(mb.usage_for("mod_c"), 0);
    }

    #[test]
    fn update_existing_module() {
        let mut mb = MemoryBudget::new(1000, PathBuf::from("/tmp/test"));
        mb.update_usage("mod_a", 400);
        mb.update_usage("mod_a", 600);
        assert_eq!(mb.usage_for("mod_a"), 600);
        assert_eq!(mb.tracked_count(), 1);
    }

    #[test]
    fn budget_exceeded() {
        let mut mb = MemoryBudget::new(100, PathBuf::from("/tmp/test"));
        mb.update_usage("mod_a", 60);
        mb.update_usage("mod_b", 50);
        assert!(mb.is_exceeded());
        assert_eq!(mb.remaining(), 0);
    }

    #[test]
    fn remove_module() {
        let mut mb = MemoryBudget::new(1000, PathBuf::from("/tmp/test"));
        mb.update_usage("mod_a", 400);
        mb.update_usage("mod_b", 300);
        mb.remove("mod_a");
        assert_eq!(mb.usage_for("mod_a"), 0);
        assert_eq!(mb.total_usage(), 300);
        assert_eq!(mb.tracked_count(), 1);
    }

    #[test]
    fn eviction_candidates_order() {
        let mut mb = MemoryBudget::new(100, PathBuf::from("/tmp/test"));
        mb.update_usage("old_mod", 80);
        mb.update_usage("new_mod", 80);
        // trigger_order: least-recent first
        let order = vec!["old_mod".to_string(), "new_mod".to_string()];
        let candidates = mb.candidates_for_eviction(&order);
        // Should evict old_mod first
        assert!(candidates.contains(&"old_mod".to_string()));
        assert!(candidates[0] == "old_mod");
    }

    #[test]
    fn no_eviction_when_under_budget() {
        let mut mb = MemoryBudget::new(1000, PathBuf::from("/tmp/test"));
        mb.update_usage("mod_a", 100);
        let candidates = mb.candidates_for_eviction(&["mod_a".to_string()]);
        assert!(candidates.is_empty());
    }

    #[test]
    fn state_path_sanitized() {
        let mb = MemoryBudget::new(1000, PathBuf::from("/tmp/test"));
        let path = mb.state_path("my-module/v2");
        assert!(path.to_string_lossy().contains("my-module_v2.state"));
    }

    #[test]
    fn save_and_load_state() {
        let dir = std::env::temp_dir().join("wta_test_module_state");
        let _ = fs::remove_dir_all(&dir);
        let mb = MemoryBudget::new(1000, dir.clone());
        let data = b"serialized module state here";
        mb.save_state("test_mod", data).unwrap();
        let loaded = mb.load_state("test_mod").unwrap();
        assert_eq!(loaded, Some(data.to_vec()));
        mb.clear_state("test_mod").unwrap();
        let gone = mb.load_state("test_mod").unwrap();
        assert!(gone.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_state_nonexistent() {
        let mb = MemoryBudget::new(1000, PathBuf::from("/tmp/nonexistent_test"));
        let result = mb.load_state("ghost_mod").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn clear_state_nonexistent() {
        let mb = MemoryBudget::new(1000, PathBuf::from("/tmp/nonexistent_test"));
        // Should not error
        mb.clear_state("ghost_mod").unwrap();
    }

    #[test]
    fn tracked_count() {
        let mut mb = MemoryBudget::new(1000, PathBuf::from("/tmp/test"));
        assert_eq!(mb.tracked_count(), 0);
        mb.update_usage("a", 10);
        mb.update_usage("b", 20);
        assert_eq!(mb.tracked_count(), 2);
        mb.remove("a");
        assert_eq!(mb.tracked_count(), 1);
    }
}
