//! Anomaly detection for workflow shifts using distributional divergence.
//!
//! Compares the current transition distribution to the stationary distribution
//! using information-theoretic and transport-theoretic distance measures:
//!
//! - **KL divergence**: "How surprised is the model by your current behavior?"
//! - **Wasserstein distance** (ergodic transport): "How much work to transform
//!   your expected workflow into your actual one?"
//!
//! "Your command pattern just shifted — did you start a new task?"

use crate::forecast::transition_matrix::TransitionMatrix;

/// An anomaly representing a detected workflow shift.
#[derive(Debug, Clone)]
pub struct WorkflowShift {
    /// KL divergence between current and stationary distributions.
    pub kl_divergence: f64,
    /// Wasserstein-1 distance between current and stationary.
    pub wasserstein_distance: f64,
    /// Human-readable severity label.
    pub severity: ShiftSeverity,
    /// Description of the detected shift.
    pub description: String,
}

/// Severity level for workflow shifts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShiftSeverity {
    /// Within normal variation.
    Normal,
    /// Mild shift — possible context change.
    Mild,
    /// Significant — likely started a new task.
    Significant,
    /// Dramatic — completely different workflow.
    Dramatic,
}

impl std::fmt::Display for ShiftSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShiftSeverity::Normal => write!(f, "normal"),
            ShiftSeverity::Mild => write!(f, "mild"),
            ShiftSeverity::Significant => write!(f, "significant"),
            ShiftSeverity::Dramatic => write!(f, "dramatic"),
        }
    }
}

/// A time-stamped record of distributional distance.
#[derive(Debug, Clone)]
pub struct ShiftRecord {
    /// Timestamp (unix epoch seconds).
    pub timestamp_secs: u64,
    /// KL divergence at this point.
    pub kl_divergence: f64,
    /// Wasserstein distance at this point.
    pub wasserstein_distance: f64,
}

/// The anomaly detector tracks divergence from the stationary distribution over time.
#[derive(Debug, Clone)]
pub struct AnomalyDetector {
    /// History of shift measurements.
    pub history: Vec<ShiftRecord>,
    /// Maximum history length.
    pub max_history: usize,
    /// Thresholds for severity classification (KL divergence).
    pub mild_threshold: f64,
    /// Significant threshold.
    pub significant_threshold: f64,
    /// Dramatic threshold.
    pub dramatic_threshold: f64,
}

impl AnomalyDetector {
    /// Create a new detector with default thresholds.
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            max_history: 1000,
            mild_threshold: 0.5,
            significant_threshold: 1.5,
            dramatic_threshold: 3.0,
        }
    }

    /// Create a detector with custom thresholds.
    pub fn with_thresholds(mild: f64, significant: f64, dramatic: f64) -> Self {
        Self {
            history: Vec::new(),
            max_history: 1000,
            mild_threshold: mild,
            significant_threshold: significant,
            dramatic_threshold: dramatic,
        }
    }

    /// Compute KL divergence KL(P || Q) where P = current row distribution,
    /// Q = stationary distribution.
    pub fn kl_divergence(matrix: &mut TransitionMatrix, current: &str) -> f64 {
        let i = match matrix.index_of(current) {
            Some(idx) => idx,
            None => return 0.0,
        };

        let n = matrix.num_states();
        if n == 0 {
            return 0.0;
        }

        let probs = matrix.probabilities().to_vec();
        let row = &probs[i];
        let stationary = matrix.stationary_distribution();

        let mut kl = 0.0;
        for j in 0..n {
            let p = row[j];
            let q = stationary[j];
            if p > 0.0 && q > 0.0 {
                kl += p * (p / q).ln();
            }
        }
        kl
    }

    /// Compute Wasserstein-1 distance between the current row distribution
    /// and the stationary distribution.
    pub fn wasserstein_distance(matrix: &mut TransitionMatrix, current: &str) -> f64 {
        let i = match matrix.index_of(current) {
            Some(idx) => idx,
            None => return 0.0,
        };

        let n = matrix.num_states();
        if n == 0 {
            return 0.0;
        }

        let probs = matrix.probabilities().to_vec();
        let row = &probs[i];
        let stationary = matrix.stationary_distribution();

        let mut cum_diff = 0.0;
        let mut distance = 0.0;
        for j in 0..n {
            cum_diff += row[j] - stationary[j];
            distance += cum_diff.abs();
        }
        distance
    }

    /// Detect a workflow shift for the given current command.
    pub fn detect(
        &mut self,
        matrix: &mut TransitionMatrix,
        current: &str,
        timestamp_secs: u64,
    ) -> Option<WorkflowShift> {
        let kl = Self::kl_divergence(matrix, current);
        let w1 = Self::wasserstein_distance(matrix, current);

        self.history.push(ShiftRecord {
            timestamp_secs,
            kl_divergence: kl,
            wasserstein_distance: w1,
        });
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }

        let severity = if kl >= self.dramatic_threshold {
            ShiftSeverity::Dramatic
        } else if kl >= self.significant_threshold {
            ShiftSeverity::Significant
        } else if kl >= self.mild_threshold {
            ShiftSeverity::Mild
        } else {
            ShiftSeverity::Normal
        };

        let description = match severity {
            ShiftSeverity::Normal => "Command pattern within normal range.".to_string(),
            ShiftSeverity::Mild => {
                format!("Mild workflow shift detected (KL={:.2}). You might be switching context.", kl)
            }
            ShiftSeverity::Significant => {
                format!(
                    "Your command pattern just shifted — did you start a new task? (KL={:.2}, W₁={:.2})",
                    kl, w1
                )
            }
            ShiftSeverity::Dramatic => {
                format!(
                    "Dramatic workflow change! Completely different command pattern. (KL={:.2}, W₁={:.2})",
                    kl, w1
                )
            }
        };

        if severity > ShiftSeverity::Normal {
            Some(WorkflowShift {
                kl_divergence: kl,
                wasserstein_distance: w1,
                severity,
                description,
            })
        } else {
            None
        }
    }

    /// Get the full shift history.
    pub fn history(&self) -> &[ShiftRecord] {
        &self.history
    }

    /// Compute the trend: is divergence increasing or decreasing?
    pub fn divergence_trend(&self, window: usize) -> f64 {
        let recent: Vec<_> = self.history.iter().rev().take(window).collect();
        if recent.len() < 2 {
            return 0.0;
        }
        let n = recent.len() as f64;
        let x_mean = (n - 1.0) / 2.0;
        let y_mean: f64 = recent.iter().map(|r| r.kl_divergence).sum::<f64>() / n;
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        for (idx, record) in recent.iter().enumerate().rev() {
            let x = idx as f64;
            numerator += (x - x_mean) * (record.kl_divergence - y_mean);
            denominator += (x - x_mean).powi(2);
        }
        if denominator.abs() < 1e-14 {
            0.0
        } else {
            numerator / denominator
        }
    }
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_matrix() -> TransitionMatrix {
        let mut m = TransitionMatrix::with_max_states(10);
        for _ in 0..100 {
            m.record(Some("cargo build"), "cargo test");
            m.record(Some("cargo test"), "git add");
            m.record(Some("git add"), "git commit");
            m.record(Some("git commit"), "cargo build");
        }
        m
    }

    #[test]
    fn kl_divergence_normal_workflow() {
        let mut m = make_matrix();
        let kl = AnomalyDetector::kl_divergence(&mut m, "cargo build");
        assert!(kl < 1.0, "normal command should have low KL, got {}", kl);
    }

    #[test]
    fn kl_divergence_unknown_command() {
        let mut m = make_matrix();
        let kl = AnomalyDetector::kl_divergence(&mut m, "totally_unknown");
        assert_eq!(kl, 0.0);
    }

    #[test]
    fn wasserstein_normal_workflow() {
        let mut m = make_matrix();
        let w1 = AnomalyDetector::wasserstein_distance(&mut m, "cargo build");
        assert!(w1 < 1.0, "normal command should have small W1, got {}", w1);
    }

    #[test]
    fn wasserstein_unknown_command() {
        let mut m = make_matrix();
        let w1 = AnomalyDetector::wasserstein_distance(&mut m, "nonexistent");
        assert_eq!(w1, 0.0);
    }

    #[test]
    fn detect_normal_returns_none() {
        let mut m = make_matrix();
        let mut det = AnomalyDetector::new();
        let shift = det.detect(&mut m, "cargo build", 1000);
        assert!(shift.is_none());
    }

    #[test]
    fn detect_shift_after_new_pattern() {
        let mut m = make_matrix();
        for _ in 0..50 {
            m.record(Some("npm install"), "npm test");
            m.record(Some("npm test"), "npm run build");
        }
        let mut det = AnomalyDetector::new();
        let shift = det.detect(&mut m, "npm install", 2000);
        assert!(shift.is_some());
        let s = shift.unwrap();
        assert!(s.kl_divergence > 0.0);
        assert!(s.wasserstein_distance >= 0.0);
    }

    #[test]
    fn severity_classification() {
        assert!(ShiftSeverity::Normal < ShiftSeverity::Mild);
        assert!(ShiftSeverity::Mild < ShiftSeverity::Significant);
        assert!(ShiftSeverity::Significant < ShiftSeverity::Dramatic);
    }

    #[test]
    fn severity_display() {
        assert_eq!(format!("{}", ShiftSeverity::Normal), "normal");
        assert_eq!(format!("{}", ShiftSeverity::Dramatic), "dramatic");
    }

    #[test]
    fn history_tracks_records() {
        let mut m = make_matrix();
        let mut det = AnomalyDetector::new();
        det.detect(&mut m, "cargo build", 100);
        det.detect(&mut m, "cargo test", 200);
        assert_eq!(det.history().len(), 2);
        assert_eq!(det.history()[0].timestamp_secs, 100);
    }

    #[test]
    fn history_respects_max_length() {
        let mut m = make_matrix();
        let mut det = AnomalyDetector::new();
        det.max_history = 5;
        for t in 0..10u64 {
            det.detect(&mut m, "cargo build", t);
        }
        assert_eq!(det.history().len(), 5);
    }

    #[test]
    fn divergence_trend_insufficient_data() {
        let det = AnomalyDetector::new();
        assert_eq!(det.divergence_trend(5), 0.0);
    }

    #[test]
    fn divergence_trend_increasing() {
        let mut det = AnomalyDetector::new();
        for t in 0..5u64 {
            det.history.push(ShiftRecord {
                timestamp_secs: t,
                kl_divergence: t as f64,
                wasserstein_distance: 0.0,
            });
        }
        let trend = det.divergence_trend(5);
        assert!(trend > 0.0, "increasing trend should be positive, got {}", trend);
    }

    #[test]
    fn custom_thresholds() {
        let det = AnomalyDetector::with_thresholds(0.1, 0.5, 1.0);
        assert_eq!(det.mild_threshold, 0.1);
        assert_eq!(det.significant_threshold, 0.5);
        assert_eq!(det.dramatic_threshold, 1.0);
    }

    #[test]
    fn kl_divergence_for_rare_command() {
        let mut m = make_matrix();
        for _ in 0..5 {
            m.record(Some("rare_cmd"), "unusual_target");
        }
        let kl = AnomalyDetector::kl_divergence(&mut m, "rare_cmd");
        assert!(kl > 0.0, "rare command should have positive KL divergence");
    }
}
