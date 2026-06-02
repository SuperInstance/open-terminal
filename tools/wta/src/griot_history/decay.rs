//! Temporal decay model for command history.
//!
//! Each command has a "retelling strength" that decays exponentially:
//!   strength(t) = exp(-λ * age_seconds)
//!
//! Recent commands: strength ≈ 1.0
//! Commands from a week ago: strength ≈ 0.3
//!
//! BUT if you run the same command again, it STRENGTHENS all prior instances.
//! This models griot oral tradition: frequently-told stories persist longer.

use std::collections::HashMap;

/// Half-life in seconds for the exponential decay.
/// At this age, a command's strength is 0.5.
/// Default: ~5 days (432000 seconds).
const DEFAULT_HALF_LIFE_SECS: f64 = 432_000.0;

/// Decay constant λ = ln(2) / half_life
const LAMBDA: f64 = std::f64::consts::LN_2 / DEFAULT_HALF_LIFE_SECS;

/// Reinforcement factor: each retelling multiplies strength by this amount.
const RETELLING_BOOST: f64 = 0.3;

/// Minimum strength to be considered "persisting" in the barcode.
const PERSISTENCE_THRESHOLD: f64 = 0.1;

/// A normalized retelling strength value in [0.0, 1.0+].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetellingStrength(pub f64);

impl RetellingStrength {
    /// Whether this strength is above the persistence threshold.
    pub fn persists(&self) -> bool {
        self.0 >= PERSISTENCE_THRESHOLD
    }

    /// Normalize to [0, 1] clamping.
    pub fn normalized(&self) -> f64 {
        self.0.min(1.0).max(0.0)
    }
}

impl std::fmt::Display for RetellingStrength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.3}", self.0)
    }
}

/// A single recorded command instance.
#[derive(Debug, Clone)]
pub struct CommandRecord {
    /// The command string.
    pub command: String,
    /// Timestamp in seconds since epoch.
    pub timestamp: u64,
    /// Number of times this command (or an identical string) has been run
    /// at or before this timestamp. Used for retelling reinforcement.
    pub retelling_count: u32,
}

impl CommandRecord {
    /// Compute the retelling strength at the given reference time.
    pub fn strength_at(&self, reference_time: u64) -> RetellingStrength {
        let age_secs = reference_time.saturating_sub(self.timestamp) as f64;
        let decay = (-LAMBDA * age_secs).exp();
        // Each retelling adds a boost.
        let boost = 1.0 + (self.retelling_count as f64) * RETELLING_BOOST;
        RetellingStrength(decay * boost)
    }
}

/// The decay model tracking all command records.
#[derive(Debug, Clone, Default)]
pub struct DecayModel {
    /// All recorded command instances.
    records: Vec<CommandRecord>,
    /// Tracks retelling counts per command string.
    retelling_counts: HashMap<String, u32>,
    /// The reference time (latest timestamp seen).
    reference_time: u64,
}

impl DecayModel {
    /// Create a new empty decay model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a command at the given timestamp.
    /// Updates retelling counts and strengthens prior instances.
    pub fn record(&mut self, command: String, timestamp: u64) {
        let count = self.retelling_counts.entry(command.clone()).or_insert(0);
        *count += 1;

        // Strengthen all prior instances of the same command.
        for rec in &mut self.records {
            if rec.command == command {
                rec.retelling_count = *count;
            }
        }

        self.records.push(CommandRecord {
            command,
            timestamp,
            retelling_count: *count,
        });

        if timestamp > self.reference_time {
            self.reference_time = timestamp;
        }
    }

    /// Get all command records.
    pub fn records(&self) -> &[CommandRecord] {
        &self.records
    }

    /// Get the reference time (latest timestamp).
    pub fn reference_time(&self) -> u64 {
        self.reference_time
    }

    /// Compute strengths for all commands at the reference time.
    pub fn all_strengths(&self) -> Vec<(String, RetellingStrength)> {
        self.records
            .iter()
            .map(|r| (r.command.clone(), r.strength_at(self.reference_time)))
            .collect()
    }

    /// Get commands that persist (above threshold) at the reference time.
    pub fn persisting_commands(&self) -> Vec<(String, RetellingStrength)> {
        self.all_strengths()
            .into_iter()
            .filter(|(_, s)| s.persists())
            .collect()
    }

    /// Get the aggregate strength for a specific command string.
    /// Combines all instances of that command.
    pub fn command_strength(&self, command: &str) -> RetellingStrength {
        let strengths: Vec<f64> = self
            .records
            .iter()
            .filter(|r| r.command == command)
            .map(|r| r.strength_at(self.reference_time).0)
            .collect();
        if strengths.is_empty() {
            RetellingStrength(0.0)
        } else {
            // Aggregate: sum strengths but cap at 1.0 for display purposes.
            RetellingStrength(strengths.iter().sum::<f64>().min(2.0))
        }
    }

    /// Unique command strings.
    pub fn unique_commands(&self) -> Vec<String> {
        let mut cmds: Vec<String> = self.retelling_counts.keys().cloned().collect();
        cmds.sort();
        cmds
    }

    /// Number of total recorded commands.
    pub fn total_count(&self) -> usize {
        self.records.len()
    }

    /// Decay curve: strengths at evenly spaced time points.
    /// Returns (time_offset_secs, strength) pairs for visualization.
    pub fn decay_curve(&self, command: &str, num_points: usize) -> Vec<(f64, f64)> {
        if self.reference_time == 0 || num_points == 0 {
            return Vec::new();
        }
        let max_age = self.reference_time as f64;
        let step = max_age / num_points.max(1) as f64;

        let mut curve = Vec::with_capacity(num_points);
        let count = *self.retelling_counts.get(command).unwrap_or(&0);
        for i in 0..num_points {
            let age = step * i as f64;
            let decay = (-LAMBDA * age).exp();
            let boost = 1.0 + (*count as f64) * RETELLING_BOOST;
            curve.push((age, decay * boost));
        }
        curve
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(days_ago: u64) -> u64 {
        let now: u64 = 1700000000;
        now - days_ago * 86400
    }

    #[test]
    fn recent_command_full_strength() {
        let mut model = DecayModel::new();
        model.record("cargo build".into(), ts(0));
        let strengths = model.all_strengths();
        assert_eq!(strengths.len(), 1);
        assert!(strengths[0].1 .0 > 0.99);
    }

    #[test]
    fn week_old_command_decays() {
        let mut model = DecayModel::new();
        model.record("cargo build".into(), ts(7));
        let strengths = model.all_strengths();
        // With default half-life of 5 days, 7-day-old command should be around 0.3-0.4
        assert!(strengths[0].1 .0 < 0.5);
        assert!(strengths[0].1 .0 > 0.1);
    }

    #[test]
    fn retelling_strengthens_prior() {
        let mut model = DecayModel::new();
        model.record("cargo build".into(), ts(10)); // Old
        model.record("cargo build".into(), ts(0));  // Recent retelling

        // The old instance should now have retelling_count=2, giving it a boost
        let old_strength = model.records()[0].strength_at(model.reference_time());
        // Without boost, 10-day-old would be very weak (~0.07)
        // With retelling_count=2 boost, it should be stronger
        assert!(old_strength.0 > 0.1, "old command boosted by retelling: {}", old_strength.0);
    }

    #[test]
    fn persistence_threshold() {
        let strength = RetellingStrength(0.15);
        assert!(strength.persists());
        let weak = RetellingStrength(0.05);
        assert!(!weak.persists());
    }

    #[test]
    fn persisting_commands_filter() {
        let mut model = DecayModel::new();
        model.record("cargo build".into(), ts(0));   // persists
        model.record("ls".into(), ts(30));            // very old, likely not
        let persisting = model.persisting_commands();
        assert!(persisting.iter().any(|(c, _)| c == "cargo build"));
    }

    #[test]
    fn command_strength_aggregates() {
        let mut model = DecayModel::new();
        model.record("cargo build".into(), ts(0));
        model.record("cargo build".into(), ts(1));
        let s = model.command_strength("cargo build");
        // Two instances should give combined strength > 1.0 (capped at 2.0)
        assert!(s.0 > 1.0);
    }

    #[test]
    fn empty_model() {
        let model = DecayModel::new();
        assert!(model.records().is_empty());
        assert_eq!(model.total_count(), 0);
        assert!(model.unique_commands().is_empty());
    }

    #[test]
    fn decay_curve_points() {
        let mut model = DecayModel::new();
        model.record("test".into(), ts(0));
        let curve = model.decay_curve("test", 10);
        assert_eq!(curve.len(), 10);
        // First point should be near full strength
        assert!(curve[0].1 > 0.99);
        // Last point should be weakest
        assert!(curve.last().unwrap().1 < curve[0].1);
    }

    #[test]
    fn decay_curve_unknown_command() {
        let model = DecayModel::new();
        let curve = model.decay_curve("unknown", 5);
        // With no reference time, returns empty
        assert!(curve.is_empty());
    }

    #[test]
    fn unique_commands_sorted() {
        let mut model = DecayModel::new();
        model.record("zebra".into(), ts(0));
        model.record("alpha".into(), ts(0));
        model.record("mid".into(), ts(0));
        let cmds = model.unique_commands();
        assert_eq!(cmds, vec!["alpha", "mid", "zebra"]);
    }

    #[test]
    fn retelling_count_increments() {
        let mut model = DecayModel::new();
        model.record("git status".into(), ts(5));
        model.record("git status".into(), ts(3));
        model.record("git status".into(), ts(1));
        assert_eq!(model.records().len(), 3);
        // All should have retelling_count = 3
        for rec in model.records() {
            assert_eq!(rec.retelling_count, 3);
        }
    }
}
