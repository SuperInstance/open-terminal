//! # Griot-Mode Command History with Decaying Persistence
//!
//! Feature-gated under `griot-history`.
//!
//! The griot tradition: oral history that decays over time but strengthens
//! with retelling. Each command has a "retelling strength" that decays
//! exponentially — but re-running the same command strengthens all prior
//! instances, modeling how frequently-told stories persist longer in memory.
//!
//! ## Submodules
//!
//! - [`decay`] — Temporal decay model with retelling reinforcement
//! - [`pattern`] — Command pattern mining and workflow detection
//! - [`adinkra`] — Context-aware compression and alias suggestion
//! - [`persistence`] — Barcode visualization of command persistence

#[cfg(feature = "griot-history")]
pub mod adinkra;
#[cfg(feature = "griot-history")]
pub mod decay;
#[cfg(feature = "griot-history")]
pub mod pattern;
#[cfg(feature = "griot-history")]
pub mod persistence;

// Re-export the public API surface.
#[cfg(feature = "griot-history")]
pub use adinkra::{ProjectContext, AdinkraCompressor};
#[cfg(feature = "griot-history")]
pub use decay::{DecayModel, CommandRecord, RetellingStrength};
#[cfg(feature = "griot-history")]
pub use pattern::{PatternMiner, WorkflowPattern, LearningPlateau};
#[cfg(feature = "griot-history")]
pub use persistence::{PersistenceBarcode, PersistenceDiagram};

/// Top-level griot history analysis for a command log.
///
/// Given a sequence of timestamped commands, produces a complete griot-mode
/// analysis: decay strengths, pattern detection, context-aware compression,
/// and persistence visualization.
#[derive(Debug)]
#[cfg(feature = "griot-history")]
pub struct GriotAnalysis {
    /// The decay model after processing all commands.
    pub decay_model: DecayModel,
    /// Detected workflow patterns.
    pub patterns: Vec<WorkflowPattern>,
    /// Detected learning plateaus.
    pub plateaus: Vec<LearningPlateau>,
    /// Project context (if detected).
    pub project_context: Option<ProjectContext>,
    /// Persistence barcode.
    pub barcode: PersistenceBarcode,
}

#[cfg(feature = "griot-history")]
impl GriotAnalysis {
    /// Run full griot analysis on a list of command strings with timestamps.
    ///
    /// `commands` is a slice of `(command_string, timestamp_secs_since_epoch)` pairs.
    /// `project_files` is a list of filenames present in the working directory
    /// (used for project detection).
    pub fn analyze(
        commands: &[(String, u64)],
        project_files: &[&str],
    ) -> Self {
        let mut decay_model = DecayModel::new();
        for (cmd, ts) in commands {
            decay_model.record(cmd.clone(), *ts);
        }

        let pattern_miner = PatternMiner::from_commands(commands);
        let patterns = pattern_miner.detect_patterns();
        let plateaus = pattern_miner.detect_plateaus();

        let project_context = AdinkraCompressor::detect_project(project_files);

        let barcode = PersistenceBarcode::from_model(&decay_model);

        GriotAnalysis {
            decay_model,
            patterns,
            plateaus,
            project_context,
            barcode,
        }
    }
}

#[cfg(test)]
#[cfg(feature = "griot-history")]
mod tests {
    use super::*;

    fn ts(days_ago: u64) -> u64 {
        let now: u64 = 1700000000; // fixed reference
        now - days_ago * 86400
    }

    #[test]
    fn griot_analysis_basic() {
        let commands = vec![
            ("cargo build".into(), ts(0)),
            ("cargo test".into(), ts(0)),
            ("cargo build".into(), ts(1)),
            ("cargo test".into(), ts(1)),
            ("git status".into(), ts(5)),
        ];
        let files = ["Cargo.toml", "src/main.rs"];
        let analysis = GriotAnalysis::analyze(&commands, &files);

        assert!(analysis.project_context.is_some());
        assert!(!analysis.patterns.is_empty() || analysis.barcode.total_commands() > 0);
    }

    #[test]
    fn griot_analysis_empty() {
        let analysis = GriotAnalysis::analyze(&[], &[]);
        assert!(analysis.project_context.is_none());
        assert!(analysis.patterns.is_empty());
    }
}
