//! # Conservation of Verification Entropy
//!
//! Tracks the ratio of edits to test runs per session. When a developer
//! edits many lines without running tests, "verification entropy" accumulates,
//! indicating that latent bugs are likely being introduced.
//!
//! This is a thermodynamic metaphor: entropy is *conserved*. Every edit
//! performed without verification increases entropy. Running tests
//! discharges it.
//!
//! ## Model
//!
//! ```text
//! E = 1 - exp(-α · edits_since_last_test / L)
//! ```
//!
//! Where:
//! - `α` is a scaling factor (default 0.005)
//! - `L` is a reference "lines per test unit" (default 3.0)
//! - The result is clamped to [0.0, 1.0]

use serde::{Deserialize, Serialize};
use std::fmt;

/// Threshold levels for verification entropy warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntropyLevel {
    /// Green: recently verified, low risk.
    Low,
    /// Yellow: accumulating entropy, moderate risk.
    Medium,
    /// Orange: significant unverified changes, get ready to test.
    High,
    /// Red: conservation of verification entropy guarantees bugs are coming.
    Critical,
}

impl fmt::Display for EntropyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntropyLevel::Low => write!(f, "low"),
            EntropyLevel::Medium => write!(f, "medium"),
            EntropyLevel::High => write!(f, "high"),
            EntropyLevel::Critical => write!(f, "critical"),
        }
    }
}

/// A verification-related event emitted by the entropy tracker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationEvent {
    /// The current entropy value when the event was emitted.
    pub entropy: f64,
    /// Human-readable severity level.
    pub level: EntropyLevel,
    /// How many lines have been edited since the last test.
    pub edits_since_last_test: u64,
    /// Total lines edited across all sessions.
    pub total_lines_edited: u64,
    /// Total test commands run across all sessions.
    pub total_tests_run: u64,
    /// A message explaining the current state.
    pub message: String,
}

/// Tracks the edit-to-test ratio and computes verification entropy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationEntropy {
    /// The alpha decay factor for the entropy formula.
    alpha: f64,
    /// Reference "lines per test unit."
    lines_per_test_unit: f64,
    /// Edits (in lines) since the last test command was detected.
    edits_since_last_test: u64,
    /// Running total of lines edited.
    total_lines_edited: u64,
    /// Running total of test commands run.
    total_tests_run: u64,
    /// Thresholds for each entropy level.
    medium_threshold: f64,
    high_threshold: f64,
    critical_threshold: f64,
}

impl VerificationEntropy {
    /// Create a new entropy tracker with default parameters.
    ///
    /// Defaults:
    /// - α = 0.005
    /// - lines_per_test_unit = 3.0
    /// - Medium threshold = 0.30
    /// - High threshold = 0.60
    /// - Critical threshold = 0.80
    pub fn new() -> Self {
        Self {
            alpha: 0.005,
            lines_per_test_unit: 3.0,
            edits_since_last_test: 0,
            total_lines_edited: 0,
            total_tests_run: 0,
            medium_threshold: 0.30,
            high_threshold: 0.60,
            critical_threshold: 0.80,
        }
    }

    /// Create a new entropy tracker with custom parameters.
    #[allow(clippy::too_many_arguments)]
    pub fn with_params(
        alpha: f64,
        lines_per_test_unit: f64,
        medium_threshold: f64,
        high_threshold: f64,
        critical_threshold: f64,
    ) -> Self {
        Self {
            alpha,
            lines_per_test_unit,
            edits_since_last_test: 0,
            total_lines_edited: 0,
            total_tests_run: 0,
            medium_threshold,
            high_threshold,
            critical_threshold,
        }
    }

    /// Record that `lines` lines of code were edited (e.g. from a save
    /// or file-write event). Returns an event if the entropy level
    /// crossed a threshold (high or critical) and warrants attention.
    pub fn record_edit(&mut self, lines: u64) -> Option<VerificationEvent> {
        self.edits_since_last_test += lines;
        self.total_lines_edited += lines;

        let entropy_before = self.compute_entropy();
        let level_before = self.level(entropy_before);

        if level_before == EntropyLevel::High || level_before == EntropyLevel::Critical {
            Some(VerificationEvent {
                entropy: entropy_before,
                level: level_before,
                edits_since_last_test: self.edits_since_last_test,
                total_lines_edited: self.total_lines_edited,
                total_tests_run: self.total_tests_run,
                message: self.build_message(level_before),
            })
        } else {
            None
        }
    }

    /// Record that a test command was executed. Resets the
    /// `edits_since_last_test` counter and reduces entropy.
    ///
    /// Returns an event describing the discharged state.
    pub fn record_test(&mut self) -> VerificationEvent {
        let entropy_before = self.compute_entropy();
        let level_after = EntropyLevel::Low;

        self.total_tests_run += 1;
        self.edits_since_last_test = 0;

        let entropy_after = self.compute_entropy();
        let message = if entropy_before > 0.5 {
            format!(
                "Testing discharged entropy from {:.0}% to {:.0}%",
                entropy_before * 100.0,
                entropy_after * 100.0,
            )
        } else {
            "Fresh test run — entropy reset.".to_string()
        };

        VerificationEvent {
            entropy: entropy_after,
            level: level_after,
            edits_since_last_test: 0,
            total_lines_edited: self.total_lines_edited,
            total_tests_run: self.total_tests_run,
            message,
        }
    }

    /// Record a batch of edits (e.g. multiple files saved at once) and
    /// return all events that crossed warning thresholds.
    pub fn record_edits(&mut self, lines: u64) -> Vec<VerificationEvent> {
        // Break into single-line increments so threshold crossing is accurate.
        let mut events = Vec::new();
        for _ in 0..lines {
            if let Some(event) = self.record_edit(1) {
                events.push(event);
            }
        }
        events
    }

    /// Compute the current entropy value.
    pub fn compute_entropy(&self) -> f64 {
        let effective = self.edits_since_last_test as f64 / self.lines_per_test_unit;
        let raw = 1.0 - (-self.alpha * effective).exp();
        raw.clamp(0.0, 1.0)
    }

    /// The raw count of edits since the last test.
    pub fn edits_since_last_test(&self) -> u64 {
        self.edits_since_last_test
    }

    /// Total lines edited across all sessions.
    pub fn total_lines_edited(&self) -> u64 {
        self.total_lines_edited
    }

    /// Total test commands run across all sessions.
    pub fn total_tests_run(&self) -> u64 {
        self.total_tests_run
    }

    /// The ratio of total edits to total tests.
    pub fn edit_test_ratio(&self) -> f64 {
        if self.total_tests_run == 0 {
            f64::MAX
        } else {
            self.total_lines_edited as f64 / self.total_tests_run as f64
        }
    }

    /// Classify an entropy value into a severity level.
    pub fn level(&self, entropy: f64) -> EntropyLevel {
        if entropy >= self.critical_threshold {
            EntropyLevel::Critical
        } else if entropy >= self.high_threshold {
            EntropyLevel::High
        } else if entropy >= self.medium_threshold {
            EntropyLevel::Medium
        } else {
            EntropyLevel::Low
        }
    }

    /// Get the current entropy level.
    pub fn current_level(&self) -> EntropyLevel {
        self.level(self.compute_entropy())
    }

    fn build_message(&self, level: EntropyLevel) -> String {
        let pct = (self.compute_entropy() * 100.0).round() as u64;
        match level {
            EntropyLevel::Critical => {
                format!(
                    "⚠ CONSERVATION OF VERIFICATION ENTROPY: {} lines edited without testing \
                     ({pct}%). Bugs are coming. Run tests now.",
                    self.edits_since_last_test,
                )
            }
            EntropyLevel::High => {
                format!(
                    "⚠ {} lines edited without testing ({pct}%). Verification entropy says latent \
                     bugs are accumulating. Consider running tests soon.",
                    self.edits_since_last_test,
                )
            }
            EntropyLevel::Medium => {
                format!(
                    "{} lines edited without testing ({pct}%). Entropy is building — test \
                     when convenient.",
                    self.edits_since_last_test,
                )
            }
            EntropyLevel::Low => {
                format!("Good: only {} lines since last test.", self.edits_since_last_test)
            }
        }
    }

    /// Get a short status label suitable for a status bar.
    pub fn status_bar_label(&self) -> String {
        let entropy = self.compute_entropy();
        let level = self.level(entropy);
        let pct = (entropy * 100.0).round() as u64;
        let bar = self.entropy_bar_chars();
        match level {
            EntropyLevel::Critical => format!("▶ VERIFY │ {pct}% ▓▓▓▓ {bar}"),
            EntropyLevel::High => format!("▶ Verify  │ {pct}% ▓▓▓░ {bar}"),
            EntropyLevel::Medium => format!("  verify  │ {pct}% ▓▓░░ {bar}"),
            EntropyLevel::Low => format!("  verify  │ {pct}% ▓░░░ {bar}"),
        }
    }

    /// Generate a small text bar representing current entropy (5 chars).
    fn entropy_bar_chars(&self) -> String {
        let entropy = self.compute_entropy();
        let filled = ((entropy * 5.0).round() as usize).min(5);
        let bar: String = std::iter::repeat('▓').take(filled)
            .chain(std::iter::repeat('░').take(5 - filled))
            .collect();
        bar
    }
}

impl Default for VerificationEntropy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_tracker_zero_entropy() {
        let ve = VerificationEntropy::new();
        assert_eq!(ve.compute_entropy(), 0.0);
        assert_eq!(ve.current_level(), EntropyLevel::Low);
    }

    #[test]
    fn edit_increases_entropy() {
        let mut ve = VerificationEntropy::new();
        let e0 = ve.compute_entropy();
        ve.record_edit(10);
        let e1 = ve.compute_entropy();
        assert!(e1 > e0, "edits should increase entropy");
    }

    #[test]
    fn test_resets_entropy() {
        let mut ve = VerificationEntropy::new();
        ve.record_edit(500);
        assert!(ve.compute_entropy() > 0.3);
        let event = ve.record_test();
        assert_eq!(event.level, EntropyLevel::Low);
        assert_eq!(ve.compute_entropy(), 0.0);
    }

    #[test]
    fn entropy_is_bounded_01() {
        let mut ve = VerificationEntropy::new();
        for _ in 0..100 {
            ve.record_edit(50);
        }
        let e = ve.compute_entropy();
        assert!(
            (0.0..=1.0).contains(&e),
            "entropy should be clamped to [0,1]: got {e}"
        );
    }

    #[test]
    fn records_edit_and_test_counts() {
        let mut ve = VerificationEntropy::new();
        assert_eq!(ve.total_lines_edited(), 0);
        assert_eq!(ve.total_tests_run(), 0);

        ve.record_edit(42);
        assert_eq!(ve.total_lines_edited(), 42);

        ve.record_test();
        assert_eq!(ve.total_tests_run(), 1);
    }

    #[test]
    fn edit_test_ratio_computed_correctly() {
        let mut ve = VerificationEntropy::new();
        ve.record_edit(100);
        ve.record_test();
        ve.record_edit(50);
        assert_eq!(ve.total_lines_edited(), 150);
        assert_eq!(ve.total_tests_run(), 1);
        assert!((ve.edit_test_ratio() - 150.0).abs() < 0.01);
    }

    #[test]
    fn edit_test_ratio_infinite_when_no_tests() {
        let mut ve = VerificationEntropy::new();
        ve.record_edit(10);
        assert_eq!(ve.edit_test_ratio(), f64::MAX);
    }

    #[test]
    fn level_transitions_from_low_to_critical() {
        let mut ve = VerificationEntropy::new();
        assert_eq!(ve.current_level(), EntropyLevel::Low);

        // ~350 lines should push us past medium (0.30)
        ve.record_edit(350);
        assert!(
            ve.current_level() != EntropyLevel::Low,
            "350 lines should move past low: got {:?}",
            ve.current_level()
        );

        // ~800 more lines should push to high
        ve.record_edit(800);
        assert_eq!(
            ve.current_level(),
            EntropyLevel::High,
            "1150 lines should be high: got {:?}",
            ve.current_level()
        );

        ve.record_edit(1500);
        assert_eq!(
            ve.current_level(),
            EntropyLevel::Critical,
            "2650 lines should be critical: got {:?}",
            ve.current_level()
        );
    }

    #[test]
    fn record_edit_triggers_event_at_high() {
        let mut ve = VerificationEntropy::new();
        // Build up entropy
        for _ in 0..200 {
            ve.record_edit(1);
        }
        // Check that at high threshold, record_edit returns Some
        // Start fresh
        let mut ve2 = VerificationEntropy::new();
        let mut triggered = false;
        for _ in 0..700 {
            if ve2.record_edit(1).is_some() {
                triggered = true;
                break;
            }
        }
        assert!(triggered, "should emit event at high entropy");
    }

    #[test]
    fn record_test_returns_discharge_message() {
        let mut ve = VerificationEntropy::new();
        ve.record_edit(200);
        let event = ve.record_test();
        assert!(event.message.contains("discharged") || event.message.contains("reset"));
        assert_eq!(event.edits_since_last_test, 0);
    }

    #[test]
    fn record_edits_batch_produces_events() {
        let mut ve = VerificationEntropy::new();
        let events = ve.record_edits(600);
        // At some point should hit high or critical
        let has_warning = events
            .iter()
            .any(|e| e.level == EntropyLevel::High || e.level == EntropyLevel::Critical);
        assert!(has_warning, "batch of 600 edits should trigger warnings");
    }

    #[test]
    fn status_bar_label_includes_percentage() {
        let mut ve = VerificationEntropy::new();
        ve.record_edit(50);
        let label = ve.status_bar_label();
        assert!(!label.is_empty());
        assert!(label.contains('%'));
    }

    #[test]
    fn custom_params_affect_entropy() {
        let mut ve = VerificationEntropy::with_params(0.01, 3.0, 0.3, 0.6, 0.8);
        ve.record_edit(100);
        let e = ve.compute_entropy();
        let mut default = VerificationEntropy::new();
        default.record_edit(100);
        let e_default = default.compute_entropy();
        // Higher alpha = faster entropy growth
        assert!(
            e > e_default,
            "higher alpha should produce higher entropy: {e} vs {e_default}"
        );
    }

    #[test]
    fn entropy_bar_chars_scales() {
        let ve = VerificationEntropy::new();
        let bar = ve.entropy_bar_chars();
        assert_eq!(bar.chars().count(), 5, "bar should be 5 chars wide, got {} chars", bar.chars().count());
        // At zero entropy, all should be unfilled
        assert_eq!(bar, "░░░░░");
    }

    #[test]
    fn display_format() {
        assert_eq!(format!("{}", EntropyLevel::Low), "low");
        assert_eq!(format!("{}", EntropyLevel::Medium), "medium");
        assert_eq!(format!("{}", EntropyLevel::High), "high");
        assert_eq!(format!("{}", EntropyLevel::Critical), "critical");
    }

    #[test]
    fn multiple_tests_preserve_counters() {
        let mut ve = VerificationEntropy::new();
        ve.record_edit(50);
        ve.record_test();
        ve.record_edit(30);
        ve.record_test();
        ve.record_test();
        assert_eq!(ve.total_tests_run(), 3);
        assert_eq!(ve.total_lines_edited(), 80);
    }

    #[test]
    fn edits_since_last_test_is_accurate() {
        let mut ve = VerificationEntropy::new();
        assert_eq!(ve.edits_since_last_test(), 0);
        ve.record_edit(15);
        assert_eq!(ve.edits_since_last_test(), 15);
        ve.record_test();
        assert_eq!(ve.edits_since_last_test(), 0);
        ve.record_edit(7);
        assert_eq!(ve.edits_since_last_test(), 7);
    }
}
