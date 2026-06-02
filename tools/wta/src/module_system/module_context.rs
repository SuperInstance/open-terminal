//! Constrained context that modules can access.
//!
//! Modules are guests in the terminal — they see only what we choose to
//! expose. They cannot access agent internals, the filesystem, or the
//! network directly.

/// Maximum number of recent commands exposed to modules.
pub const COMMAND_HISTORY_WINDOW: usize = 200;

/// A read-only snapshot of the terminal state relevant to modules.
#[derive(Debug, Clone)]
pub struct ModuleContext {
    /// Recent command history (last N commands, newest last).
    pub command_history: Vec<CommandEntry>,
    /// Current working directory.
    pub working_directory: String,
    /// IDs of active agent sessions (opaque strings, no internals).
    pub active_agent_ids: Vec<String>,
    /// Last error message, if any.
    pub last_error: Option<String>,
    /// Last exit code.
    pub last_exit_code: Option<i32>,
    /// Project files detected in the working directory.
    pub project_files: Vec<String>,
}

impl ModuleContext {
    /// Create an empty context.
    pub fn empty() -> Self {
        Self {
            command_history: Vec::new(),
            working_directory: String::new(),
            active_agent_ids: Vec::new(),
            last_error: None,
            last_exit_code: None,
            project_files: Vec::new(),
        }
    }

    /// Create a context from the given data, truncating history to the
    /// window size.
    pub fn new(
        command_history: Vec<CommandEntry>,
        working_directory: String,
        active_agent_ids: Vec<String>,
        last_error: Option<String>,
        last_exit_code: Option<i32>,
        project_files: Vec<String>,
    ) -> Self {
        // Trim to the window, keeping the most recent entries.
        let command_history = if command_history.len() > COMMAND_HISTORY_WINDOW {
            command_history[command_history.len() - COMMAND_HISTORY_WINDOW..].to_vec()
        } else {
            command_history
        };

        Self {
            command_history,
            working_directory,
            active_agent_ids,
            last_error,
            last_exit_code,
            project_files,
        }
    }

    /// Number of commands in the history window.
    pub fn history_len(&self) -> usize {
        self.command_history.len()
    }

    /// Whether there is an active error.
    pub fn has_error(&self) -> bool {
        self.last_error.is_some()
    }

    /// Whether there are any active agent sessions.
    pub fn has_active_agents(&self) -> bool {
        !self.active_agent_ids.is_empty()
    }

    /// Project files as `&str` slices (for APIs that take `&[&str]`).
    pub fn project_files_strs(&self) -> Vec<&str> {
        self.project_files.iter().map(|s| s.as_str()).collect()
    }

    /// Command strings from history.
    pub fn command_strings(&self) -> Vec<String> {
        self.command_history.iter().map(|e| e.command.clone()).collect()
    }

    /// Command-timestamp pairs for analysis modules.
    pub fn command_timestamp_pairs(&self) -> Vec<(String, u64)> {
        self.command_history
            .iter()
            .map(|e| (e.command.clone(), e.timestamp_secs))
            .collect()
    }
}

/// A single command entry exposed to modules.
#[derive(Debug, Clone)]
pub struct CommandEntry {
    /// The command string.
    pub command: String,
    /// Timestamp in seconds since epoch.
    pub timestamp_secs: u64,
    /// Exit code (0 = success, nonzero = error).
    pub exit_code: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(cmd: &str, ts: u64) -> CommandEntry {
        CommandEntry {
            command: cmd.to_string(),
            timestamp_secs: ts,
            exit_code: 0,
        }
    }

    #[test]
    fn empty_context() {
        let ctx = ModuleContext::empty();
        assert!(ctx.command_history.is_empty());
        assert!(ctx.working_directory.is_empty());
        assert!(ctx.active_agent_ids.is_empty());
        assert!(ctx.last_error.is_none());
        assert!(ctx.last_exit_code.is_none());
        assert!(ctx.project_files.is_empty());
    }

    #[test]
    fn new_context_basic() {
        let entries = vec![make_entry("ls", 100), make_entry("pwd", 200)];
        let ctx = ModuleContext::new(
            entries,
            "/home/user".to_string(),
            vec!["agent-1".to_string()],
            None,
            Some(0),
            vec!["Cargo.toml".to_string()],
        );
        assert_eq!(ctx.history_len(), 2);
        assert_eq!(ctx.working_directory, "/home/user");
        assert!(ctx.has_active_agents());
        assert!(!ctx.has_error());
    }

    #[test]
    fn history_truncated_to_window() {
        let entries: Vec<CommandEntry> = (0..300)
            .map(|i| make_entry(&format!("cmd_{}", i), i as u64))
            .collect();
        let ctx = ModuleContext::new(entries, "/tmp".into(), vec![], None, None, vec![]);
        assert_eq!(ctx.history_len(), COMMAND_HISTORY_WINDOW);
        // The most recent entries should be kept.
        assert_eq!(ctx.command_history[0].command, "cmd_100");
    }

    #[test]
    fn has_error() {
        let ctx = ModuleContext::new(
            vec![],
            "/tmp".into(),
            vec![],
            Some("permission denied".into()),
            Some(1),
            vec![],
        );
        assert!(ctx.has_error());
        assert_eq!(ctx.last_error.as_deref(), Some("permission denied"));
    }

    #[test]
    fn project_files_strs() {
        let ctx = ModuleContext::new(
            vec![],
            "/tmp".into(),
            vec![],
            None,
            None,
            vec!["Cargo.toml".into(), "src/main.rs".into()],
        );
        assert_eq!(ctx.project_files_strs(), vec!["Cargo.toml", "src/main.rs"]);
    }

    #[test]
    fn command_strings() {
        let ctx = ModuleContext::new(
            vec![make_entry("ls", 1), make_entry("pwd", 2)],
            "/tmp".into(),
            vec![],
            None,
            None,
            vec![],
        );
        assert_eq!(ctx.command_strings(), vec!["ls", "pwd"]);
    }

    #[test]
    fn command_timestamp_pairs() {
        let ctx = ModuleContext::new(
            vec![make_entry("ls", 100), make_entry("pwd", 200)],
            "/tmp".into(),
            vec![],
            None,
            None,
            vec![],
        );
        let pairs = ctx.command_timestamp_pairs();
        assert_eq!(pairs, vec![("ls".into(), 100), ("pwd".into(), 200)]);
    }

    #[test]
    fn no_active_agents() {
        let ctx = ModuleContext::empty();
        assert!(!ctx.has_active_agents());
    }
}
