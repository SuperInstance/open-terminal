//! # Skill Detection via Renormalization
//!
//! Feature-gated under `griot-history`.
//!
//! The core idea from renormalization-group theory applied to command histories:
//! your command history is a signal at fine grain. Coarse-grain it (renormalize)
//! and see what survives. **What survives IS your skill.**
//!
//! ## Submodules
//!
//! - [`coarse_grain`] — Block-spin transformation for command sequences
//! - [`fixed_point`] — Detecting skill plateaus (fixed points of RG flow)
//! - [`universality`] — Universality classes of workflows based on critical exponents
//! - [`suggestion`] — Workflow optimization suggestions
//!
//! ## How it works
//!
//! 1. Take the last N commands from history (a "signal")
//! 2. Group into blocks, replace each block with its representative (most frequent command)
//! 3. Repeat on the coarse-grained signal (renormalization flow)
//! 4. After 3-4 levels, patterns that survive at every scale are *fixed points* — mastered skills
//! 5. The convergence rate (critical exponent) tells you *how hard* the skill was to learn
//! 6. Classify workflows into universality classes based on their exponents
//!
//! "Learning IS coarse-graining. A mastered skill is a fixed point.
//!  The critical exponent tells you how hard it was to learn."

#[cfg(feature = "griot-history")]
pub mod coarse_grain;
#[cfg(feature = "griot-history")]
pub mod fixed_point;
#[cfg(feature = "griot-history")]
pub mod universality;
#[cfg(feature = "griot-history")]
pub mod suggestion;

#[cfg(feature = "griot-history")]
pub use coarse_grain::{CoarseGrainer, CoarseGrainLevel, BlockSize};
#[cfg(feature = "griot-history")]
pub use fixed_point::{FixedPointDetector, FixedPoint, ConvergenceInfo};
#[cfg(feature = "griot-history")]
pub use universality::{UniversalityClassifier, UniversalityClass, CriticalExponent};
#[cfg(feature = "griot-history")]
pub use suggestion::{SuggestionEngine, Suggestion, SuggestionKind};

/// A single command in the history, potentially with metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg(feature = "griot-history")]
pub struct CommandEntry {
    /// The command string (normalized: trimmed, lowercased).
    pub command: String,
    /// Optional timestamp (seconds since epoch).
    pub timestamp: Option<u64>,
}

#[cfg(feature = "griot-history")]
impl CommandEntry {
    /// Create a new command entry from a raw string.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into().trim().to_string(),
            timestamp: None,
        }
    }

    /// Create with a timestamp.
    pub fn with_timestamp(command: impl Into<String>, ts: u64) -> Self {
        Self {
            command: command.into().trim().to_string(),
            timestamp: Some(ts),
        }
    }
}

/// Complete skill detection analysis result.
#[derive(Debug, Clone)]
#[cfg(feature = "griot-history")]
pub struct SkillAnalysis {
    /// Detected fixed-point skills.
    pub skills: Vec<FixedPoint>,
    /// The universality class of the overall workflow.
    pub universality_class: UniversalityClass,
    /// The critical exponent measuring workflow convergence.
    pub critical_exponent: CriticalExponent,
    /// Generated suggestions based on the analysis.
    pub suggestions: Vec<Suggestion>,
    /// Number of renormalization levels processed.
    pub levels_processed: usize,
    /// Total commands analyzed.
    pub command_count: usize,
}

/// Run a complete skill detection analysis on a command history.
#[cfg(feature = "griot-history")]
pub fn analyze(commands: &[CommandEntry]) -> SkillAnalysis {
    analyze_with_config(commands, &DefaultConfig::default())
}

/// Configuration for skill detection analysis.
#[derive(Debug, Clone)]
#[cfg(feature = "griot-history")]
pub struct DefaultConfig {
    /// Block sizes for coarse-graining (applied sequentially).
    pub block_sizes: Vec<BlockSize>,
    /// Maximum renormalization levels.
    pub max_levels: usize,
    /// Convergence threshold (JSD below this = fixed point).
    pub convergence_threshold: f64,
}

#[cfg(feature = "griot-history")]
impl Default for DefaultConfig {
    fn default() -> Self {
        Self {
            block_sizes: vec![BlockSize::B2, BlockSize::B5, BlockSize::B10],
            max_levels: 6,
            convergence_threshold: 0.01,
        }
    }
}

/// Run skill detection with custom configuration.
#[cfg(feature = "griot-history")]
pub fn analyze_with_config(commands: &[CommandEntry], config: &DefaultConfig) -> SkillAnalysis {
    let command_strings: Vec<String> = commands.iter().map(|c| c.command.clone()).collect();

    // Step 1: Coarse-grain through multiple levels
    let grainer = CoarseGrainer::new(&config.block_sizes);
    let levels = grainer.renormalize(&command_strings, config.max_levels);

    // Step 2: Detect fixed points
    let detector = FixedPointDetector::new(config.convergence_threshold);
    let skills = detector.detect(&levels);

    // Step 3: Classify universality
    let classifier = UniversalityClassifier::new();
    let exponent = classifier.compute_critical_exponent(&levels);
    let universality_class = classifier.classify(&exponent);

    // Step 4: Generate suggestions
    let engine = SuggestionEngine::new();
    let suggestions = engine.generate(&skills, &universality_class, &exponent);

    SkillAnalysis {
        skills,
        universality_class,
        critical_exponent: exponent,
        suggestions,
        levels_processed: levels.len(),
        command_count: commands.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_entry_trims_whitespace() {
        let entry = CommandEntry::new("  git status  ");
        assert_eq!(entry.command, "git status");
    }

    #[test]
    fn command_entry_with_timestamp() {
        let entry = CommandEntry::with_timestamp("cargo build", 1000);
        assert_eq!(entry.command, "cargo build");
        assert_eq!(entry.timestamp, Some(1000));
    }

    #[test]
    fn empty_history_produces_no_skills() {
        let analysis = analyze(&[]);
        assert!(analysis.skills.is_empty());
        assert_eq!(analysis.command_count, 0);
    }

    #[test]
    fn uniform_commands_detect_single_skill() {
        // 50 identical commands → single fixed point
        let commands: Vec<CommandEntry> = (0..50)
            .map(|_| CommandEntry::new("git status"))
            .collect();
        let analysis = analyze(&commands);
        assert!(!analysis.skills.is_empty());
        // The surviving skill should be "git status"
        let skill_commands: Vec<&str> = analysis.skills.iter().map(|s| s.command.as_str()).collect();
        assert!(skill_commands.contains(&"git status"));
    }

    #[test]
    fn two_cycle_workflow() {
        // Alternating build/test → both should survive
        let commands: Vec<CommandEntry> = (0..50)
            .flat_map(|i| {
                vec![
                    CommandEntry::new("cargo build"),
                    CommandEntry::new("cargo test"),
                ]
            })
            .chain(std::iter::once(CommandEntry::new("cargo build")))
            .collect();
        let analysis = analyze(&commands);
        assert!(analysis.skills.len() >= 2);
    }

    #[test]
    fn analysis_with_config_custom_threshold() {
        let commands: Vec<CommandEntry> = (0..20)
            .map(|i| CommandEntry::new(if i % 3 == 0 { "git commit" } else { "git add" }))
            .collect();
        let config = DefaultConfig {
            convergence_threshold: 0.001,
            ..Default::default()
        };
        let analysis = analyze_with_config(&commands, &config);
        assert_eq!(analysis.command_count, 20);
    }

    #[test]
    fn skill_analysis_has_suggestions() {
        let commands: Vec<CommandEntry> = (0..100)
            .map(|i| CommandEntry::new(if i % 2 == 0 { "make" } else { "make test" }))
            .collect();
        let analysis = analyze(&commands);
        assert!(!analysis.suggestions.is_empty());
    }
}
