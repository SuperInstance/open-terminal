//! Detecting skill plateaus — fixed points of the renormalization flow.
//!
//! A "skill" in our framework is a command pattern that's a fixed point
//! of the renormalization flow. When coarse-graining stops changing the
//! signal, the user has converged on a stable workflow.
//!
//! "You've been doing the same git workflow for 3 months — here's a script for it."
//!
//! The convergence rate (critical exponent) tells us how deep the skill is:
//! - Fast convergence = shallow learning (rote repetition)
//! - Slow convergence = deep skill (hard-won mastery)

use super::coarse_grain::CoarseGrainLevel;
use std::collections::HashMap;

/// A detected fixed point — a command that survives coarse-graining.
#[derive(Debug, Clone, PartialEq)]
pub struct FixedPoint {
    /// The command that survived.
    pub command: String,
    /// Level at which it first appeared as a fixed point.
    pub first_fixed_level: usize,
    /// How many consecutive levels it survived unchanged.
    pub survival_length: usize,
    /// The relative frequency at the fixed-point level.
    pub frequency: f64,
    /// Whether this fixed point is still active (survived to the last level).
    pub active: bool,
}

impl FixedPoint {
    /// Quality score: higher = more stable, more dominant skill.
    pub fn quality(&self) -> f64 {
        let survival_bonus = (self.survival_length as f64).ln() + 1.0;
        self.frequency * survival_bonus
    }
}

/// Information about convergence of the renormalization flow.
#[derive(Debug, Clone)]
pub struct ConvergenceInfo {
    /// The level at which the flow first converged.
    pub converged_at_level: Option<usize>,
    /// Number of levels that were identical to their predecessor.
    pub identical_transitions: usize,
    /// Total levels examined.
    pub total_levels: usize,
    /// Jensen-Shannon divergence between successive levels.
    pub jsd_history: Vec<f64>,
}

impl ConvergenceInfo {
    /// Whether the flow fully converged.
    pub fn is_converged(&self) -> bool {
        self.converged_at_level.is_some()
    }

    /// The critical exponent: rate of convergence.
    ///
    /// Computed as the negative slope of JSD vs level on a log scale.
    /// Large exponent → fast convergence (shallow learning).
    /// Small exponent → slow convergence (deep skill).
    pub fn critical_exponent(&self) -> f64 {
        if self.jsd_history.len() < 2 {
            return 0.0;
        }
        // Fit log(JSD) vs level; slope is the exponent
        let valid: Vec<(f64, f64)> = self
            .jsd_history
            .iter()
            .enumerate()
            .filter(|(_, &jsd)| jsd > 1e-10)
            .map(|(i, &jsd)| (i as f64, jsd.ln()))
            .collect();

        if valid.len() < 2 {
            return 0.0;
        }

        let n = valid.len() as f64;
        let sum_x: f64 = valid.iter().map(|(x, _)| x).sum();
        let sum_y: f64 = valid.iter().map(|(_, y)| y).sum();
        let sum_xy: f64 = valid.iter().map(|(x, y)| x * y).sum();
        let sum_x2: f64 = valid.iter().map(|(x, _)| x * x).sum();

        let denom = n * sum_x2 - sum_x * sum_x;
        if denom.abs() < 1e-10 {
            return 0.0;
        }

        // Negative slope = decay rate = critical exponent
        let slope = (n * sum_xy - sum_x * sum_y) / denom;
        -slope
    }
}

/// Detects fixed points in a sequence of coarse-grained levels.
#[derive(Debug, Clone)]
pub struct FixedPointDetector {
    /// JSD threshold below which we consider two distributions identical.
    convergence_threshold: f64,
}

impl FixedPointDetector {
    pub fn new(convergence_threshold: f64) -> Self {
        Self {
            convergence_threshold,
        }
    }

    /// Detect fixed points across a sequence of coarse-grained levels.
    pub fn detect(&self, levels: &[CoarseGrainLevel]) -> Vec<FixedPoint> {
        if levels.len() < 2 {
            return Vec::new();
        }

        // Track which commands survive across levels
        let mut candidates: HashMap<String, FixedPointBuilder> = HashMap::new();

        for level in levels {
            let total = level.commands.len().max(1) as f64;

            for cmd in &level.commands {
                let freq = level.distribution.get(cmd).copied().unwrap_or(0) as f64 / total;

                let builder = candidates.entry(cmd.clone()).or_insert_with(|| {
                    FixedPointBuilder {
                        command: cmd.clone(),
                        first_seen_level: level.level,
                        current_streak: 0,
                        max_streak: 0,
                        last_level: None,
                        last_freq: 0.0,
                        active: true,
                    }
                });

                if builder.last_level == Some(level.level - 1) {
                    builder.current_streak += 1;
                } else if builder.last_level.map_or(true, |l| l < level.level - 1) {
                    // Gap — reset streak but keep candidate alive
                    builder.current_streak = 1;
                }
                builder.max_streak = builder.max_streak.max(builder.current_streak);
                builder.last_level = Some(level.level);
                builder.last_freq = freq;
                builder.active = true;
            }

            // Mark commands not in this level as inactive
            let level_commands: std::collections::HashSet<&str> =
                level.commands.iter().map(|s| s.as_str()).collect();
            for builder in candidates.values_mut() {
                if !level_commands.contains(builder.command.as_str()) {
                    builder.active = false;
                }
            }
        }

        // Convert builders to FixedPoints: only commands that survived 2+ levels
        candidates
            .into_values()
            .filter(|b| b.max_streak >= 1) // Survived at least one transition
            .map(|b| FixedPoint {
                command: b.command,
                first_fixed_level: b.first_seen_level,
                survival_length: b.max_streak,
                frequency: b.last_freq,
                active: b.active,
            })
            .collect()
    }

    /// Compute convergence info for the level sequence.
    pub fn convergence_info(&self, levels: &[CoarseGrainLevel]) -> ConvergenceInfo {
        let mut jsd_history = Vec::new();
        let mut converged_at = None;
        let mut identical_transitions = 0;

        for window in levels.windows(2) {
            let jsd = self.jsd(&window[0], &window[1]);
            jsd_history.push(jsd);

            if jsd < self.convergence_threshold && converged_at.is_none() {
                converged_at = Some(window[1].level);
            }

            if jsd < 1e-10 {
                identical_transitions += 1;
            }
        }

        ConvergenceInfo {
            converged_at_level: converged_at,
            identical_transitions,
            total_levels: levels.len(),
            jsd_history,
        }
    }

    /// Jensen-Shannon divergence between two levels' command distributions.
    fn jsd(&self, a: &CoarseGrainLevel, b: &CoarseGrainLevel) -> f64 {
        let all_keys: std::collections::HashSet<&str> = a
            .distribution
            .keys()
            .chain(b.distribution.keys())
            .map(|s| s.as_str())
            .collect();

        let a_total = a.commands.len().max(1) as f64;
        let b_total = b.commands.len().max(1) as f64;

        let mut jsd = 0.0;
        for key in &all_keys {
            let p = a.distribution.get(*key).copied().unwrap_or(0) as f64 / a_total;
            let q = b.distribution.get(*key).copied().unwrap_or(0) as f64 / b_total;
            let m = (p + q) / 2.0;

            if p > 0.0 {
                jsd += p * (p / m).log2() / 2.0;
            }
            if q > 0.0 {
                jsd += q * (q / m).log2() / 2.0;
            }
        }

        jsd
    }
}

/// Helper for tracking fixed-point candidates across levels.
struct FixedPointBuilder {
    command: String,
    first_seen_level: usize,
    current_streak: usize,
    max_streak: usize,
    last_level: Option<usize>,
    last_freq: f64,
    active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::griot_history::skill_detector::coarse_grain::{CoarseGrainer, BlockSize};

    fn make_levels(commands: &[&str], max_levels: usize) -> Vec<CoarseGrainLevel> {
        let cmds: Vec<String> = commands.iter().map(|s| s.to_string()).collect();
        let grainer = CoarseGrainer::new(&[BlockSize::B2, BlockSize::B5]);
        grainer.renormalize(&cmds, max_levels)
    }

    #[test]
    fn single_uniform_command_is_fixed_point() {
        let commands = vec!["git status"; 32];
        let levels = make_levels(&commands, 5);
        let detector = FixedPointDetector::new(0.01);
        let fps = detector.detect(&levels);
        assert!(fps.iter().any(|fp| fp.command == "git status" && fp.active));
    }

    #[test]
    fn empty_levels_no_fixed_points() {
        let detector = FixedPointDetector::new(0.01);
        let fps = detector.detect(&[]);
        assert!(fps.is_empty());
    }

    #[test]
    fn one_level_no_fixed_points() {
        let levels = make_levels(&["a", "b", "a"], 0);
        let detector = FixedPointDetector::new(0.01);
        let fps = detector.detect(&levels);
        assert!(fps.is_empty()); // Need 2+ levels for fixed points
    }

    #[test]
    fn convergence_info_single_level() {
        let levels = make_levels(&["a"; 4], 0);
        let detector = FixedPointDetector::new(0.01);
        let info = detector.convergence_info(&levels);
        assert!(!info.is_converged()); // No transitions
        assert!(info.jsd_history.is_empty());
    }

    #[test]
    fn uniform_converges_immediately() {
        let commands = vec!["make"; 64];
        let levels = make_levels(&commands, 5);
        let detector = FixedPointDetector::new(0.01);
        let info = detector.convergence_info(&levels);
        // Uniform signal should have near-zero JSD from level 1 onward
        assert!(info.is_converged());
    }

    #[test]
    fn critical_exponent_positive_for_converging() {
        let commands: Vec<&str> = (0..100)
            .map(|i| if i % 20 < 2 { "noise" } else { "cargo build" })
            .collect();
        let levels = make_levels(&commands, 5);
        let detector = FixedPointDetector::new(0.01);
        let info = detector.convergence_info(&levels);
        // Should have a positive exponent (converging)
        assert!(info.critical_exponent() >= 0.0);
    }

    #[test]
    fn fixed_point_quality_score() {
        let fp = FixedPoint {
            command: "git commit".to_string(),
            first_fixed_level: 1,
            survival_length: 4,
            frequency: 0.8,
            active: true,
        };
        assert!(fp.quality() > 0.0);
    }

    #[test]
    fn two_cycle_detection() {
        let commands: Vec<&str> = (0..40)
            .map(|i| if i % 2 == 0 { "build" } else { "test" })
            .collect();
        let levels = make_levels(&commands, 5);
        let detector = FixedPointDetector::new(0.01);
        let fps = detector.detect(&levels);
        // Both "build" and "test" should appear as candidates
        let cmds: Vec<&str> = fps.iter().map(|fp| fp.command.as_str()).collect();
        assert!(cmds.contains(&"build"));
        assert!(cmds.contains(&"test"));
    }

    #[test]
    fn jsd_between_identical_distributions_is_zero() {
        let detector = FixedPointDetector::new(0.01);
        let a = CoarseGrainLevel::new(0, 1, vec!["a".to_string(); 10]);
        let b = CoarseGrainLevel::new(1, 2, vec!["a".to_string(); 5]);
        let jsd = detector.jsd(&a, &b);
        assert!(jsd < 1e-10);
    }

    #[test]
    fn jsd_between_different_distributions_is_positive() {
        let detector = FixedPointDetector::new(0.01);
        let a = CoarseGrainLevel::new(0, 1, vec!["a".to_string(); 10]);
        let b = CoarseGrainLevel::new(1, 2, vec!["b".to_string(); 10]);
        let jsd = detector.jsd(&a, &b);
        assert!(jsd > 0.5);
    }

    #[test]
    fn convergence_info_tracks_identical_transitions() {
        let commands = vec!["x"; 64];
        let levels = make_levels(&commands, 5);
        let detector = FixedPointDetector::new(0.01);
        let info = detector.convergence_info(&levels);
        assert!(info.identical_transitions >= 1);
    }

    #[test]
    fn survival_length_increases_with_stability() {
        let commands = vec!["stable"; 128];
        let levels = make_levels(&commands, 6);
        let detector = FixedPointDetector::new(0.01);
        let fps = detector.detect(&levels);
        let stable = fps.iter().find(|fp| fp.command == "stable").unwrap();
        assert!(stable.survival_length >= 3);
    }
}
