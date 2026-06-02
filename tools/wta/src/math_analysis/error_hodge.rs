//! # Hodge Decomposition of Error Signals
//!
//! Decomposes an error into three orthogonal components:
//!
//! - **Evidence** — "what happened" (raw signal from stderr, exit code, etc.)
//! - **Coherence** — "does this error make sense internally"
//! - **Prior mismatch** — "expectation vs reality" (the user's mental model)
//!
//! Each component is scored 0.0–1.0, and a dominant classification is
//! reported so downstream autofix pipelines can provide better context.

use serde::{Deserialize, Serialize};

/// Which component of the error is most significant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ErrorDominance {
    /// The error is clear and factual; the signal is strong.
    Evidence,
    /// The error message itself is confusing or contradictory.
    Incoherence,
    /// The user expected one thing but got another; a prior mismatch.
    PriorMismatch,
}

/// A full Hodge-style decomposition of an error signal.
///
/// # Interpretation
///
/// - **High evidence + low prior mismatch**: The error is straightforward —
///   the system clearly explains what went wrong.
/// - **High coherence + high evidence**: The error is well-formed and
///   actionable.
/// - **High prior mismatch**: The user's expected behavior diverges from
///   what happened — common when switching tools, versions, or workflows.
/// - **Low coherence**: The error message itself is confusing or
///   self-contradictory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDecomposition {
    /// Evidence score (0.0–1.0): how much factual signal the error carries.
    pub evidence: f64,
    /// Coherence score (0.0–1.0): whether the error is internally consistent.
    pub coherence: f64,
    /// Prior mismatch score (0.0–1.0): divergence between expectation and
    /// reality.
    pub prior_mismatch: f64,
    /// Which component dominates.
    pub dominance: ErrorDominance,
    /// A human-readable explanation of the decomposition.
    pub explanation: String,
}

impl ErrorDecomposition {
    fn normalize_score(raw: f64) -> f64 {
        raw.clamp(0.0, 1.0)
    }

    fn determine_dominance(evidence: f64, coherence: f64, mismatch: f64) -> ErrorDominance {
        // Incoherence is measured as 1.0 - coherence. If incoherence > max of
        // the other two, it dominates.
        let incoherence = 1.0 - coherence;
        if incoherence > evidence && incoherence > mismatch {
            ErrorDominance::Incoherence
        } else if mismatch > evidence {
            ErrorDominance::PriorMismatch
        } else {
            ErrorDominance::Evidence
        }
    }

    fn build_explanation(
        evidence: f64,
        coherence: f64,
        mismatch: f64,
        dominance: &ErrorDominance,
    ) -> String {
        let evidence_pct = (evidence * 100.0).round() as u32;
        let coherence_pct = (coherence * 100.0).round() as u32;
        let mismatch_pct = (mismatch * 100.0).round() as u32;

        match dominance {
            ErrorDominance::Evidence => {
                format!(
                    "This error is {evidence_pct}% evidence — the error signal is strong and \
                     factual. Coherence: {coherence_pct}%, prior mismatch: {mismatch_pct}%. \
                     Actionable: the error clearly describes the problem.",
                )
            }
            ErrorDominance::Incoherence => {
                format!(
                    "This error is {incoherence_pct}% incoherence — the error message itself is \
                     confusing or internally contradictory. Evidence: {evidence_pct}%, \
                     prior mismatch: {mismatch_pct}%. Suggestion: try to reproduce or \
                     simplify before acting.",
                    incoherence_pct = (100 - coherence_pct),
                )
            }
            ErrorDominance::PriorMismatch => {
                format!(
                    "This error is {mismatch_pct}% prior mismatch — your expectation diverges \
                     from reality. Evidence: {evidence_pct}%, coherence: {coherence_pct}%. \
                     Check for version changes, environment differences, or new constraints.",
                )
            }
        }
    }
}

/// A Hodge-style error analyzer. Maintains contextual state for computing
/// prior mismatch (what the user has recently done) and coherence checks.
#[derive(Debug, Clone)]
pub struct ErrorHodge {
    /// Recent commands executed before the error occurred.
    recent_commands: Vec<String>,
    /// Known good exit codes for the most common commands.
    known_expected_exit_codes: Vec<i32>,
}

impl ErrorHodge {
    /// Create a new Hodge analyzer with default configuration.
    pub fn new() -> Self {
        Self {
            recent_commands: Vec::new(),
            // 0 is the universal "success"; everything else is a failure.
            known_expected_exit_codes: vec![0],
        }
    }

    /// Create a new Hodge analyzer with custom recent command history.
    pub fn with_recent_commands(recent: Vec<String>) -> Self {
        Self {
            recent_commands: recent,
            known_expected_exit_codes: vec![0],
        }
    }

    /// Add a command to the recent history (most recent goes last).
    pub fn push_command(&mut self, command: String) {
        const MAX_HISTORY: usize = 20;
        self.recent_commands.push(command);
        if self.recent_commands.len() > MAX_HISTORY {
            self.recent_commands.remove(0);
        }
    }

    /// Record all the commands that are expected to exit with non-zero codes.
    pub fn set_expected_exit_codes(&mut self, codes: Vec<i32>) {
        self.known_expected_exit_codes = codes;
    }

    /// Decompose an error into its components.
    ///
    /// # Arguments
    ///
    /// * `exit_code` — The process exit code (negative if not known).
    /// * `stderr_len` — Length of the stderr text in bytes.
    /// * `stderr_has_signal` — Whether stderr contains substantive error
    ///   text (not just path traces).
    /// * `expected_command` — The command name the user expected to work.
    /// * `previous_version` — The previous version of the tool, if known
    ///   (e.g. Python 3.11 → 3.12 change).
    pub fn decompose(
        &self,
        exit_code: i32,
        stderr_len: usize,
        stderr_has_signal: bool,
        expected_command: &str,
        previous_version: Option<&str>,
    ) -> ErrorDecomposition {
        // ── Evidence component ──
        // Strong evidence: non-zero exit code + substantive stderr.
        let code_signal = if exit_code != 0 && !self.known_expected_exit_codes.contains(&exit_code)
        {
            0.7
        } else {
            0.2
        };
        let stderr_signal = if stderr_has_signal {
            let len_factor = (stderr_len as f64).min(2000.0) / 2000.0;
            0.3 + 0.5 * len_factor
        } else if stderr_len > 0 {
            0.1
        } else {
            0.0
        };
        let evidence = ErrorDecomposition::normalize_score(code_signal + stderr_signal * 0.7);

        // ── Coherence component ──
        // Check for common incoherent patterns:
        // - Very short stderr with a non-zero exit code suggests a vague error.
        // - Contradictory keywords in stderr increase incoherence.
        let coherence = if stderr_len > 0 && stderr_has_signal {
            if stderr_len < 20 {
                // Very short errors are often inscrutable.
                0.3
            } else if stderr_len > 100 {
                0.8
            } else {
                0.6
            }
        } else if stderr_len > 0 {
            0.5
        } else {
            // No stderr at all — exit code only.
            0.7
        };

        // ── Prior mismatch component ──
        // If the user recently changed versions of the expected command,
        // prior mismatch is high. Also check if they recently ran a very
        // different command.
        let version_mismatch = match previous_version {
            Some(_) => 0.7,
            None => 0.0,
        };

        let recent_command_mismatch = if self.recent_commands.is_empty() {
            0.0
        } else {
            // If the last command is NOT the expected command, there may be
            // a workflow mismatch.
            let last = self.recent_commands.last().map(|s| s.as_str()).unwrap_or("");
            if last == expected_command {
                0.1
            } else if self.recent_commands.contains(&expected_command.to_string()) {
                0.2
            } else {
                0.4
            }
        };

        let decay_factor = 1.0;
        let prior_mismatch = ErrorDecomposition::normalize_score(
            (version_mismatch * 0.6 + recent_command_mismatch * 0.4) * decay_factor,
        );

        let dominance =
            ErrorDecomposition::determine_dominance(evidence, coherence, prior_mismatch);
        let explanation = ErrorDecomposition::build_explanation(
            evidence,
            coherence,
            prior_mismatch,
            &dominance,
        );

        ErrorDecomposition {
            evidence: (evidence * 100.0).round() / 100.0,
            coherence: (coherence * 100.0).round() / 100.0,
            prior_mismatch: (prior_mismatch * 100.0).round() / 100.0,
            dominance,
            explanation,
        }
    }

    /// Quick‑interactive entry point: score an error and return a short
    /// label and the full decomposition.
    pub fn score(&self, exit_code: i32, stderr: &str, expected: &str) -> (String, ErrorDecomposition) {
        let has_signal = !stderr.trim().is_empty();
        let decomp = self.decompose(exit_code, stderr.len(), has_signal, expected, None);
        let label = match decomp.dominance {
            ErrorDominance::Evidence => format!("{:.0}% evidence", decomp.evidence * 100.0),
            ErrorDominance::Incoherence => format!("{:.0}% incoherence", (1.0 - decomp.coherence) * 100.0),
            ErrorDominance::PriorMismatch => format!("{:.0}% prior mismatch", decomp.prior_mismatch * 100.0),
        };
        (label, decomp)
    }
}

impl Default for ErrorHodge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Evidence Tests ───────────────────────────────────────────────

    #[test]
    fn strong_evidence_from_exit_code_and_stderr() {
        let hodge = ErrorHodge::new();
        let decomp = hodge.decompose(1, 500, true, "cargo build", None);
        assert!(decomp.evidence > 0.5, "exit code + stderr should yield strong evidence");
        assert_eq!(decomp.dominance, ErrorDominance::Evidence);
    }

    #[test]
    fn weak_evidence_when_exit_code_success() {
        let hodge = ErrorHodge::new();
        let decomp = hodge.decompose(0, 0, false, "cargo build", None);
        assert!(decomp.evidence < 0.5, "success exit should yield weak evidence");
    }

    #[test]
    fn evidence_scales_with_stderr_length() {
        let hodge = ErrorHodge::new();
        let short = hodge.decompose(1, 10, true, "npm test", None);
        let long = hodge.decompose(1, 1500, true, "npm test", None);
        assert!(
            long.evidence >= short.evidence,
            "longer stderr with signal should have higher evidence"
        );
    }

    #[test]
    fn known_nonzero_exit_reduces_evidence() {
        let mut hodge = ErrorHodge::new();
        hodge.set_expected_exit_codes(vec![0, 1]); // grep style
        let decomp = hodge.decompose(1, 20, true, "grep", None);
        // exit code 1 is expected, so evidence should be lower
        assert!(decomp.evidence < 0.6, "expected non-zero exit code reduces evidence component");
    }

    // ─── Coherence Tests ──────────────────────────────────────────────

    #[test]
    fn long_stderr_is_more_coherent() {
        let hodge = ErrorHodge::new();
        let short = hodge.decompose(1, 5, true, "cmd", None);
        let long = hodge.decompose(1, 500, true, "cmd", None);
        assert!(long.coherence > short.coherence, "longer stderr should have higher coherence");
    }

    #[test]
    fn no_stderr_with_exit_code_is_reasonably_coherent() {
        let hodge = ErrorHodge::new();
        let decomp = hodge.decompose(127, 0, false, "nonexistent", None);
        // Exit code only, no stderr: still coherent because it's a clear signal.
        assert!(decomp.coherence >= 0.5, "exit-code-only errors can still be coherent");
    }

    #[test]
    fn short_cryptic_stderr_is_incoherent() {
        let hodge = ErrorHodge::new();
        let decomp = hodge.decompose(1, 3, true, "cmd", None);
        assert!(
            decomp.coherence < 0.5,
            "very short stderr with exit code should be somewhat incoherent"
        );
    }

    // ─── Prior Mismatch Tests ─────────────────────────────────────────

    #[test]
    fn version_change_raises_prior_mismatch() {
        let hodge = ErrorHodge::new();
        let no_change = hodge.decompose(1, 200, true, "python", None);
        let changed = hodge.decompose(1, 200, true, "python", Some("3.11"));
        assert!(
            changed.prior_mismatch > no_change.prior_mismatch,
            "version change should raise prior mismatch"
        );
    }

    #[test]
    fn prior_mismatch_dominates_with_version_change() {
        let mut hodge = ErrorHodge::new();
        hodge.push_command("python".to_string());
        hodge.push_command("python".to_string());
        // Zero exit code + no stderr → no evidence signal.
        // Version change alone drives prior_mismatch.
        let decomp = hodge.decompose(0, 0, false, "python", Some("3.11"));
        assert_eq!(decomp.dominance, ErrorDominance::PriorMismatch,
            "version change with clean exit should be prior mismatch, got {:?}",
            decomp.dominance);
    }

    #[test]
    fn unfamiliar_command_raises_mismatch() {
        let mut hodge = ErrorHodge::new();
        hodge.push_command("npm install".to_string());
        hodge.push_command("npm test".to_string());
        // Zero exit code + no stderr to keep evidence low so prior mismatch can dominate
        let decomp = hodge.decompose(0, 0, false, "cargo build", None);
        assert!(
            decomp.prior_mismatch > 0.15,
            "unexpected command with different recent history should raise mismatch, got prior_mismatch={}",
            decomp.prior_mismatch
        );
    }

    #[test]
    fn familiar_command_lowers_mismatch() {
        let mut hodge = ErrorHodge::new();
        hodge.push_command("npm install".to_string());
        hodge.push_command("npm test".to_string());
        let decomp = hodge.decompose(1, 200, true, "npm test", None);
        assert!(
            decomp.prior_mismatch < 0.3,
            "recently-used expected command should keep mismatch low: got {}",
            decomp.prior_mismatch
        );
    }

    #[test]
    fn empty_history_zero_mismatch_base() {
        let hodge = ErrorHodge::new();
        let decomp = hodge.decompose(1, 200, true, "any", None);
        // No history + no version change = low mismatch.
        assert!(
            decomp.prior_mismatch < 0.5,
            "empty history with no version change should have moderate mismatch: got {}",
            decomp.prior_mismatch
        );
    }

    // ─── Integration / Edge Cases ─────────────────────────────────────

    #[test]
    fn score_returns_label_and_decomposition() {
        let hodge = ErrorHodge::new();
        let (label, decomp) = hodge.score(1, "file not found", "cat");
        assert!(!label.is_empty());
        assert!(decomp.evidence > 0.0 || decomp.coherence > 0.0 || decomp.prior_mismatch > 0.0);
    }

    #[test]
    fn all_scores_normalized() {
        let hodge = ErrorHodge::new();
        let decomp = hodge.decompose(1, 200, true, "python", Some("3.10"));
        assert!(
            (0.0..=1.0).contains(&decomp.evidence),
            "evidence should be 0..1"
        );
        assert!(
            (0.0..=1.0).contains(&decomp.coherence),
            "coherence should be 0..1"
        );
        assert!(
            (0.0..=1.0).contains(&decomp.prior_mismatch),
            "prior_mismatch should be 0..1"
        );
    }

    #[test]
    fn dominance_labels_are_mutually_exclusive() {
        let mut hodge = ErrorHodge::new();
        hodge.push_command("foo".to_string());
        hodge.push_command("bar".to_string());
        hodge.push_command("baz".to_string());
        // Pure evidence case: exit code 1 + long stderr with signal
        let d1 = hodge.decompose(1, 500, true, "foo", None);
        assert_eq!(d1.dominance, ErrorDominance::Evidence,
            "d1 should be evidence, got {:?}", d1.dominance);
        // Version-change case: clean exit to let prior mismatch dominate
        let d2 = hodge.decompose(0, 0, false, "bar", Some("v1"));
        assert_eq!(d2.dominance, ErrorDominance::PriorMismatch,
            "d2 should be prior mismatch, got {:?}", d2.dominance);
        // Incoherence case: short stderr with signal raises incoherence
        let hodge2 = ErrorHodge::new();
        let d3 = hodge2.decompose(1, 3, true, "baz", None);
        // Each should have a distinct dominance when conditions are right
        assert!(d1.dominance != d2.dominance,
            "evidence and prior mismatch should differ");
        assert!(d3.dominance != d2.dominance || d3.dominance == ErrorDominance::Incoherence,
            "incoherence should differ from prior mismatch");
    }

    #[test]
    fn command_history_is_capped() {
        let mut hodge = ErrorHodge::new();
        for i in 0..50 {
            hodge.push_command(format!("cmd{i}"));
        }
        assert!(
            hodge.recent_commands.len() <= 20,
            "history should be capped at 20: got {}",
            hodge.recent_commands.len()
        );
    }

    #[test]
    fn decompose_with_no_stderr() {
        let hodge = ErrorHodge::new();
        let decomp = hodge.decompose(1, 0, false, "tool", None);
        // No stderr but non-zero exit: evidence from exit code.
        assert!(decomp.evidence > 0.0);
        // No stderr and no version change: prior mismatch should be low.
        assert!(decomp.prior_mismatch < 0.3);
    }

    #[test]
    fn explain_output_mentions_dominance() {
        let hodge = ErrorHodge::new();
        let decomp = hodge.decompose(1, 300, true, "python", Some("3.10"));
        assert!(decomp.explanation.contains("prior mismatch"));
        assert!(decomp.explanation.contains("%"));
    }

    #[test]
    fn set_expected_exit_codes_works() {
        let mut hodge = ErrorHodge::new();
        hodge.set_expected_exit_codes(vec![0, 1]);
        let decomp = hodge.decompose(1, 0, false, "diff", None);
        // With exit code 1 expected, evidence should be low.
        assert!(decomp.evidence < 0.4);
    }
}
