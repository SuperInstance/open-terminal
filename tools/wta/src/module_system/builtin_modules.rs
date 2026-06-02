//! Builtin module wrappers for the existing analysis modules.
//!
//! Each wraps a concrete analysis type behind the [`TerminalModule`] trait
//! so the registry can manage them uniformly.
//!
//! Modules are feature-gated:
//! - `math-tools`: CommandMarkov, ErrorHodge, VerificationEntropy, SpectralDashboard
//! - `griot-history`: GriotDecay, PatternMiner, AdinkraCompression, PersistenceViz

use super::{
    ModuleContext, ModuleOutput, TerminalModule,
    module_context::CommandEntry,
};

// ---------------------------------------------------------------------------
// 1. CommandMarkov (math_analysis)
// ---------------------------------------------------------------------------

/// Wraps `CommandMarkovChain` as a terminal module.
///
/// Triggers on `CommandEntered` events. Produces status bar output
/// showing the number of tracked commands and transitions.
#[cfg(feature = "math-tools")]
pub struct CommandMarkovModule {
    chain: crate::math_analysis::CommandMarkovChain,
    active: bool,
    prev_command: Option<String>,
}

#[cfg(feature = "math-tools")]
impl CommandMarkovModule {
    pub fn new() -> Self {
        Self {
            chain: crate::math_analysis::CommandMarkovChain::new(),
            active: false,
            prev_command: None,
        }
    }
}

#[cfg(feature = "math-tools")]
impl TerminalModule for CommandMarkovModule {
    fn id(&self) -> &str {
        "command_markov"
    }

    fn trigger(&self, event: &super::TerminalEvent) -> bool {
        matches!(event, super::TerminalEvent::CommandEntered { .. })
    }

    fn activate(&mut self, ctx: &ModuleContext) {
        // Seed the chain from existing history via record_sequence.
        let cmds: Vec<&str> = ctx.command_history.iter().map(|e| e.command.as_str()).collect();
        self.chain.record_sequence(&cmds);
        if let Some(last) = cmds.last() {
            self.prev_command = Some(last.to_string());
        }
        self.active = true;
    }

    fn handle_event(&mut self, event: &super::TerminalEvent) -> Vec<ModuleOutput> {
        if let super::TerminalEvent::CommandEntered { command, .. } = event {
            let prev = self.prev_command.as_deref();
            self.chain.record_transition(prev, command);
            self.prev_command = Some(command.clone());
            let n = self.chain.num_commands();
            let total = self.chain.total_transitions();
            vec![ModuleOutput::status_bar(format!("markov:{}cmds/{}trans", n, total))]
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
        // 512x512 u64 matrix ≈ 2 MB
        2 * 1024 * 1024
    }
}

// ---------------------------------------------------------------------------
// 2. ErrorHodge (math_analysis)
// ---------------------------------------------------------------------------

/// Wraps `ErrorHodge` as a terminal module.
///
/// Triggers on `Error` and `CommandCompleted` (nonzero exit) events.
#[cfg(feature = "math-tools")]
pub struct ErrorHodgeModule {
    hodge: crate::math_analysis::ErrorHodge,
    active: bool,
}

#[cfg(feature = "math-tools")]
impl ErrorHodgeModule {
    pub fn new() -> Self {
        Self {
            hodge: crate::math_analysis::ErrorHodge::new(),
            active: false,
        }
    }
}

#[cfg(feature = "math-tools")]
impl TerminalModule for ErrorHodgeModule {
    fn id(&self) -> &str {
        "error_hodge"
    }

    fn trigger(&self, event: &super::TerminalEvent) -> bool {
        match event {
            super::TerminalEvent::Error { .. } => true,
            super::TerminalEvent::CommandCompleted { exit_code, .. } => *exit_code != 0,
            _ => false,
        }
    }

    fn activate(&mut self, _ctx: &ModuleContext) {
        self.active = true;
    }

    fn handle_event(&mut self, event: &super::TerminalEvent) -> Vec<ModuleOutput> {
        match event {
            super::TerminalEvent::Error { message, exit_code } => {
                let decomp = self.hodge.decompose(
                    *exit_code,
                    message.len(),
                    message.to_lowercase().contains("signal"),
                    "",
                    None,
                );
                vec![ModuleOutput::Insight(format!(
                    "Hodge: evidence={:.2} coherence={:.2} mismatch={:.2} [{:?}]",
                    decomp.evidence, decomp.coherence, decomp.prior_mismatch, decomp.dominance
                ))]
            }
            super::TerminalEvent::CommandCompleted { command, exit_code, .. } if *exit_code != 0 => {
                let msg = format!("{} (exit {})", command, exit_code);
                let decomp = self.hodge.decompose(
                    *exit_code,
                    msg.len(),
                    false,
                    "",
                    None,
                );
                vec![ModuleOutput::notification(format!(
                    "Error Hodge: {:?} — {:.0}% evidence",
                    decomp.dominance,
                    decomp.evidence * 100.0
                ))]
            }
            _ => vec![],
        }
    }

    fn deactivate(&mut self) {
        self.active = false;
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn memory_usage(&self) -> usize {
        64 * 1024 // 64 KB
    }
}

// ---------------------------------------------------------------------------
// 3. VerificationEntropy (math_analysis)
// ---------------------------------------------------------------------------

/// Wraps `VerificationEntropy` as a terminal module.
///
/// Triggers on `CommandEntered` and `CommandCompleted` events.
/// Tracks edit-to-test ratios and warns about accumulating entropy.
#[cfg(feature = "math-tools")]
pub struct VerificationEntropyModule {
    entropy: crate::math_analysis::VerificationEntropy,
    active: bool,
}

#[cfg(feature = "math-tools")]
impl VerificationEntropyModule {
    pub fn new() -> Self {
        Self {
            entropy: crate::math_analysis::VerificationEntropy::new(),
            active: false,
        }
    }

    fn is_test_command(cmd: &str) -> bool {
        let lower = cmd.to_lowercase();
        lower.contains("test")
            || lower.contains("spec")
            || lower.contains("check")
            || lower.contains("verify")
    }

    fn is_edit_command(cmd: &str) -> bool {
        let lower = cmd.to_lowercase();
        lower.contains("vim")
            || lower.contains("nano")
            || lower.contains("edit")
            || lower.contains("sed")
            || lower.contains("write")
            || lower.contains("cargo build")
            || lower.contains("npm run build")
    }
}

#[cfg(feature = "math-tools")]
impl TerminalModule for VerificationEntropyModule {
    fn id(&self) -> &str {
        "verification_entropy"
    }

    fn trigger(&self, event: &super::TerminalEvent) -> bool {
        matches!(event,
            super::TerminalEvent::CommandEntered { .. }
            | super::TerminalEvent::CommandCompleted { .. }
        )
    }

    fn activate(&mut self, _ctx: &ModuleContext) {
        self.active = true;
    }

    fn handle_event(&mut self, event: &super::TerminalEvent) -> Vec<ModuleOutput> {
        if let super::TerminalEvent::CommandEntered { command, .. } = event {
            if Self::is_test_command(command) {
                self.entropy.record_test();
            } else if Self::is_edit_command(command) {
                self.entropy.record_edit(5);
            }

            let level = self.entropy.current_level();
            let bar = self.entropy.status_bar_label();

            let mut outputs = vec![ModuleOutput::status_bar(bar)];
            if matches!(level, crate::math_analysis::verification_entropy::EntropyLevel::High | crate::math_analysis::verification_entropy::EntropyLevel::Critical) {
                outputs.push(ModuleOutput::notification(format!(
                    "Verification entropy {:?}: run tests soon", level
                )));
            }
            outputs
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
        32 * 1024
    }
}

// ---------------------------------------------------------------------------
// 4. SpectralDashboard (math_analysis)
// ---------------------------------------------------------------------------

/// Wraps `SpectralDashboard` as a terminal module.
///
/// Triggers on agent session events. Produces spectral metrics for the
/// status bar.
#[cfg(feature = "math-tools")]
pub struct SpectralDashboardModule {
    dashboard: crate::math_analysis::SpectralDashboard,
    active: bool,
}

#[cfg(feature = "math-tools")]
impl SpectralDashboardModule {
    pub fn new() -> Self {
        Self {
            dashboard: crate::math_analysis::SpectralDashboard::new(),
            active: false,
        }
    }
}

#[cfg(feature = "math-tools")]
impl TerminalModule for SpectralDashboardModule {
    fn id(&self) -> &str {
        "spectral_dashboard"
    }

    fn trigger(&self, event: &super::TerminalEvent) -> bool {
        matches!(event,
            super::TerminalEvent::AgentStarted { .. }
            | super::TerminalEvent::AgentEnded { .. }
        )
    }

    fn activate(&mut self, ctx: &ModuleContext) {
        // Seed from existing agent IDs.
        for id in &ctx.active_agent_ids {
            self.dashboard.graph.add_node(&id, &id, true);
        }
        self.active = true;
    }

    fn handle_event(&mut self, event: &super::TerminalEvent) -> Vec<ModuleOutput> {
        match event {
            super::TerminalEvent::AgentStarted { agent_id } => {
                self.dashboard.graph.add_node(agent_id, agent_id, true);
            }
            super::TerminalEvent::AgentEnded { agent_id } => {
                // Mark as not alive rather than removing (preserves graph structure).
                for node in &mut self.dashboard.graph.nodes {
                    if node.id == *agent_id {
                        node.alive = false;
                    }
                }
                self.dashboard.graph.invalidate_cache();
            }
            _ => {}
        }

        self.dashboard.recompute();
        let fiedler = self.dashboard.last_fiedler.unwrap_or(0.0);
        let cheeger = self.dashboard.last_cheeger.unwrap_or(0.0);

        vec![ModuleOutput::status_bar(format!("λ₂={:.2} h={:.2}", fiedler, cheeger))]
    }

    fn deactivate(&mut self) {
        self.active = false;
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn memory_usage(&self) -> usize {
        1 * 1024 * 1024 // ~1 MB for matrices
    }
}

// ---------------------------------------------------------------------------
// 5. GriotDecay (griot_history)
// ---------------------------------------------------------------------------

/// Wraps `DecayModel` as a terminal module.
///
/// Triggers on `CommandEntered`. Tracks command retelling strengths.
#[cfg(feature = "griot-history")]
pub struct GriotDecayModule {
    model: crate::griot_history::DecayModel,
    active: bool,
}

#[cfg(feature = "griot-history")]
impl GriotDecayModule {
    pub fn new() -> Self {
        Self {
            model: crate::griot_history::DecayModel::new(),
            active: false,
        }
    }
}

#[cfg(feature = "griot-history")]
impl TerminalModule for GriotDecayModule {
    fn id(&self) -> &str {
        "griot_decay"
    }

    fn trigger(&self, event: &super::TerminalEvent) -> bool {
        matches!(event, super::TerminalEvent::CommandEntered { .. })
    }

    fn activate(&mut self, ctx: &ModuleContext) {
        for entry in &ctx.command_history {
            self.model.record(entry.command.clone(), entry.timestamp_secs);
        }
        self.active = true;
    }

    fn handle_event(&mut self, event: &super::TerminalEvent) -> Vec<ModuleOutput> {
        if let super::TerminalEvent::CommandEntered { command, timestamp_secs } = event {
            self.model.record(command.clone(), *timestamp_secs);
            let persisting = self.model.persisting_commands();
            let ratio = if self.model.total_count() > 0 {
                persisting.len() as f64 / self.model.total_count() as f64
            } else {
                0.0
            };
            vec![ModuleOutput::status_bar(format!(
                "decay:{}cmds/{:.0}%persist",
                self.model.total_count(),
                ratio * 100.0
            ))]
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
        self.model.total_count() * 128 + 4096
    }

    fn serialize_state(&self) -> Vec<u8> {
        let mut buf = String::new();
        for rec in self.model.records() {
            buf.push_str(&format!(
                "{}\t{}\t{}\n",
                rec.command, rec.timestamp, rec.retelling_count
            ));
        }
        buf.into_bytes()
    }

    fn deserialize_state(&mut self, data: &[u8]) -> bool {
        let s = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => return false,
        };
        for line in s.lines() {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() >= 2 {
                let cmd = parts[0].to_string();
                let ts: u64 = parts[1].parse().unwrap_or(0);
                self.model.record(cmd, ts);
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// 6. PatternMiner (griot_history)
// ---------------------------------------------------------------------------

/// Wraps `PatternMiner` as a terminal module.
///
/// Triggers on `CommandEntered`. Detects workflow patterns.
#[cfg(feature = "griot-history")]
pub struct PatternMinerModule {
    commands: Vec<(String, u64)>,
    active: bool,
}

#[cfg(feature = "griot-history")]
impl PatternMinerModule {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            active: false,
        }
    }
}

#[cfg(feature = "griot-history")]
impl TerminalModule for PatternMinerModule {
    fn id(&self) -> &str {
        "pattern_miner"
    }

    fn trigger(&self, event: &super::TerminalEvent) -> bool {
        matches!(event, super::TerminalEvent::CommandEntered { .. })
    }

    fn activate(&mut self, ctx: &ModuleContext) {
        self.commands = ctx.command_timestamp_pairs();
        self.active = true;
    }

    fn handle_event(&mut self, event: &super::TerminalEvent) -> Vec<ModuleOutput> {
        if let super::TerminalEvent::CommandEntered { command, timestamp_secs } = event {
            self.commands.push((command.clone(), *timestamp_secs));

            let miner = crate::griot_history::PatternMiner::from_commands(&self.commands);
            let patterns = miner.detect_patterns();

            if patterns.is_empty() {
                vec![]
            } else {
                let top = &patterns[0];
                vec![ModuleOutput::InlineHint(format!(
                    "pattern: {} ({}×, {:.0}% conf)",
                    top.label(),
                    top.frequency,
                    top.confidence * 100.0
                ))]
            }
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
        self.commands.len() * 64 + 4096
    }
}

// ---------------------------------------------------------------------------
// 7. AdinkraCompression (griot_history)
// ---------------------------------------------------------------------------

/// Wraps `AdinkraCompressor` as a terminal module.
///
/// Triggers on `ProjectFilesDetected` and `CommandEntered`.
#[cfg(feature = "griot-history")]
pub struct AdinkraCompressionModule {
    compressor: crate::griot_history::AdinkraCompressor,
    context: Option<crate::griot_history::ProjectContext>,
    active: bool,
}

#[cfg(feature = "griot-history")]
impl AdinkraCompressionModule {
    pub fn new() -> Self {
        Self {
            compressor: crate::griot_history::AdinkraCompressor::new(),
            context: None,
            active: false,
        }
    }
}

#[cfg(feature = "griot-history")]
impl TerminalModule for AdinkraCompressionModule {
    fn id(&self) -> &str {
        "adinkra_compression"
    }

    fn trigger(&self, event: &super::TerminalEvent) -> bool {
        matches!(event,
            super::TerminalEvent::ProjectFilesDetected { .. }
            | super::TerminalEvent::CommandEntered { .. }
        )
    }

    fn activate(&mut self, ctx: &ModuleContext) {
        let files: Vec<&str> = ctx.project_files.iter().map(|s| s.as_str()).collect();
        self.context = crate::griot_history::AdinkraCompressor::detect_project(&files);
        self.active = true;
    }

    fn handle_event(&mut self, event: &super::TerminalEvent) -> Vec<ModuleOutput> {
        match event {
            super::TerminalEvent::ProjectFilesDetected { files } => {
                let file_strs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
                self.context = crate::griot_history::AdinkraCompressor::detect_project(&file_strs);
                if let Some(ref ctx) = self.context {
                    vec![ModuleOutput::status_bar(format!("project:{}", ctx.kind))]
                } else {
                    vec![]
                }
            }
            super::TerminalEvent::CommandEntered { command, .. } => {
                if let Some(ref ctx) = self.context {
                    let compressed = self.compressor.compress_commands(
                        &[command.clone()],
                        ctx,
                    );
                    if let Some((alias, _)) = compressed.first() {
                        vec![ModuleOutput::InlineHint(format!("alias: {} → {}", alias, command))]
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn deactivate(&mut self) {
        self.active = false;
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn memory_usage(&self) -> usize {
        16 * 1024
    }
}

// ---------------------------------------------------------------------------
// 8. PersistenceViz (griot_history)
// ---------------------------------------------------------------------------

/// Wraps `PersistenceBarcode` as a terminal module.
///
/// Triggers on `CommandEntered`. Produces barcode visualization.
#[cfg(feature = "griot-history")]
pub struct PersistenceVizModule {
    model: crate::griot_history::DecayModel,
    active: bool,
}

#[cfg(feature = "griot-history")]
impl PersistenceVizModule {
    pub fn new() -> Self {
        Self {
            model: crate::griot_history::DecayModel::new(),
            active: false,
        }
    }
}

#[cfg(feature = "griot-history")]
impl TerminalModule for PersistenceVizModule {
    fn id(&self) -> &str {
        "persistence_viz"
    }

    fn trigger(&self, event: &super::TerminalEvent) -> bool {
        matches!(event, super::TerminalEvent::CommandEntered { .. })
    }

    fn activate(&mut self, ctx: &ModuleContext) {
        for entry in &ctx.command_history {
            self.model.record(entry.command.clone(), entry.timestamp_secs);
        }
        self.active = true;
    }

    fn handle_event(&mut self, event: &super::TerminalEvent) -> Vec<ModuleOutput> {
        if let super::TerminalEvent::CommandEntered { command, timestamp_secs } = event {
            self.model.record(command.clone(), *timestamp_secs);
            let barcode = crate::griot_history::PersistenceBarcode::from_model(&self.model);
            let ascii = barcode.render_ascii(40);
            vec![ModuleOutput::status_bar(format!("barcode:{}", ascii))]
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
        self.model.total_count() * 128 + 8192
    }
}

// ---------------------------------------------------------------------------
// Registration helper
// ---------------------------------------------------------------------------

/// Register all builtin modules into the registry.
///
/// Only modules whose feature gate is enabled are included.
#[cfg(feature = "module-system")]
pub fn register_all(registry: &mut super::ModuleRegistry) {
    #[cfg(feature = "math-tools")]
    registry.register(Box::new(CommandMarkovModule::new()));
    #[cfg(feature = "math-tools")]
    registry.register(Box::new(ErrorHodgeModule::new()));
    #[cfg(feature = "math-tools")]
    registry.register(Box::new(VerificationEntropyModule::new()));
    #[cfg(feature = "math-tools")]
    registry.register(Box::new(SpectralDashboardModule::new()));
    #[cfg(feature = "griot-history")]
    registry.register(Box::new(GriotDecayModule::new()));
    #[cfg(feature = "griot-history")]
    registry.register(Box::new(PatternMinerModule::new()));
    #[cfg(feature = "griot-history")]
    registry.register(Box::new(AdinkraCompressionModule::new()));
    #[cfg(feature = "griot-history")]
    registry.register(Box::new(PersistenceVizModule::new()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(days_ago: u64) -> u64 {
        1700000000 - days_ago * 86400
    }

    fn make_ctx(history: Vec<(&str, u64)>) -> ModuleContext {
        let entries: Vec<CommandEntry> = history
            .into_iter()
            .map(|(cmd, ts)| CommandEntry {
                command: cmd.to_string(),
                timestamp_secs: ts,
                exit_code: 0,
            })
            .collect();
        ModuleContext::new(entries, "/tmp".into(), vec![], None, None, vec![])
    }

    // --- GriotDecayModule tests ---

    #[cfg(feature = "griot-history")]
    #[test]
    fn griot_decay_basic() {
        let mut m = GriotDecayModule::new();
        assert_eq!(m.id(), "griot_decay");
        assert!(!m.is_active());
        let ctx = make_ctx(vec![("cargo build", ts(0)), ("cargo test", ts(1))]);
        m.activate(&ctx);
        assert!(m.is_active());
        let outputs = m.handle_event(&super::super::TerminalEvent::CommandEntered {
            command: "cargo build".into(),
            timestamp_secs: ts(0),
        });
        assert!(!outputs.is_empty());
        m.deactivate();
        assert!(!m.is_active());
    }

    #[cfg(feature = "griot-history")]
    #[test]
    fn griot_decay_memory_usage() {
        let m = GriotDecayModule::new();
        assert!(m.memory_usage() >= 4096);
    }

    #[cfg(feature = "griot-history")]
    #[test]
    fn griot_decay_serialize_roundtrip() {
        let mut m = GriotDecayModule::new();
        m.model.record("cargo build".into(), 1000);
        m.model.record("cargo test".into(), 2000);
        let state = m.serialize_state();
        assert!(!state.is_empty());

        let mut m2 = GriotDecayModule::new();
        assert!(m2.deserialize_state(&state));
    }

    #[cfg(feature = "griot-history")]
    #[test]
    fn griot_decay_bad_deserialize() {
        let mut m = GriotDecayModule::new();
        assert!(!m.deserialize_state(&[0xFF, 0xFE, 0xFD]));
    }

    // --- PatternMinerModule tests ---

    #[cfg(feature = "griot-history")]
    #[test]
    fn pattern_miner_no_pattern_single() {
        let mut m = PatternMinerModule::new();
        assert_eq!(m.id(), "pattern_miner");
        let ctx = ModuleContext::empty();
        m.activate(&ctx);
        let outputs = m.handle_event(&super::super::TerminalEvent::CommandEntered {
            command: "ls".into(),
            timestamp_secs: 1000,
        });
        assert!(outputs.is_empty());
    }

    #[cfg(feature = "griot-history")]
    #[test]
    fn pattern_miner_detects_repeat() {
        let mut m = PatternMinerModule::new();
        let ctx = make_ctx(vec![
            ("cargo build", ts(0)),
            ("cargo test", ts(0)),
            ("cargo build", ts(1)),
            ("cargo test", ts(1)),
        ]);
        m.activate(&ctx);
        let outputs = m.handle_event(&super::super::TerminalEvent::CommandEntered {
            command: "cargo build".into(),
            timestamp_secs: ts(0),
        });
        assert!(!outputs.is_empty());
        assert!(matches!(outputs[0], ModuleOutput::InlineHint(_)));
    }

    // --- AdinkraCompressionModule tests ---

    #[cfg(feature = "griot-history")]
    #[test]
    fn adinkra_no_project() {
        let mut m = AdinkraCompressionModule::new();
        assert_eq!(m.id(), "adinkra_compression");
        let ctx = ModuleContext::empty();
        m.activate(&ctx);
        let outputs = m.handle_event(&super::super::TerminalEvent::CommandEntered {
            command: "cargo build".into(),
            timestamp_secs: 1000,
        });
        assert!(outputs.is_empty());
    }

    #[cfg(feature = "griot-history")]
    #[test]
    fn adinkra_with_project() {
        let mut m = AdinkraCompressionModule::new();
        let ctx = ModuleContext::new(
            vec![],
            "/tmp".into(),
            vec![],
            None,
            None,
            vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
        );
        m.activate(&ctx);
        let outputs = m.handle_event(&super::super::TerminalEvent::CommandEntered {
            command: "cargo build".into(),
            timestamp_secs: 1000,
        });
        assert!(!outputs.is_empty());
    }

    #[cfg(feature = "griot-history")]
    #[test]
    fn adinkra_project_detection_event() {
        let mut m = AdinkraCompressionModule::new();
        let ctx = ModuleContext::empty();
        m.activate(&ctx);
        let outputs = m.handle_event(&super::super::TerminalEvent::ProjectFilesDetected {
            files: vec!["Cargo.toml".into()],
        });
        assert!(!outputs.is_empty());
        assert!(matches!(outputs[0], ModuleOutput::StatusBar(_)));
    }

    // --- PersistenceVizModule tests ---

    #[cfg(feature = "griot-history")]
    #[test]
    fn persistence_viz_basic() {
        let mut m = PersistenceVizModule::new();
        assert_eq!(m.id(), "persistence_viz");
        let ctx = make_ctx(vec![("cargo build", ts(0))]);
        m.activate(&ctx);
        let outputs = m.handle_event(&super::super::TerminalEvent::CommandEntered {
            command: "cargo test".into(),
            timestamp_secs: ts(0),
        });
        assert!(!outputs.is_empty());
        assert!(matches!(outputs[0], ModuleOutput::StatusBar(_)));
    }

    // --- register_all tests ---

    #[test]
    fn register_all_counts() {
        let dir = std::env::temp_dir().join("wta_test_register_all");
        let _ = std::fs::remove_dir_all(&dir);
        let mut registry = super::super::ModuleRegistry::new(
            super::super::MemoryBudget::new(50 * 1024 * 1024, dir),
        );
        register_all(&mut registry);
        #[cfg(feature = "math-tools")]
        assert!(registry.module_count() >= 4);
        #[cfg(feature = "griot-history")]
        assert!(registry.module_count() >= 4);
        #[cfg(all(feature = "math-tools", feature = "griot-history"))]
        assert_eq!(registry.module_count(), 8);
    }

    #[test]
    fn all_modules_have_unique_ids() {
        let dir = std::env::temp_dir().join("wta_test_unique_ids");
        let _ = std::fs::remove_dir_all(&dir);
        let mut registry = super::super::ModuleRegistry::new(
            super::super::MemoryBudget::new(50 * 1024 * 1024, dir),
        );
        register_all(&mut registry);
        let ids = registry.module_ids();
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(!seen.contains(*id), "duplicate module id: {}", id);
            seen.insert(id.to_string());
        }
    }
}
