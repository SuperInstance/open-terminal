//! Ergodic command analysis via Markov chain transition matrices.
//!
//! Tracks command-to-command transitions, computes the stationary
//! distribution (long-run fraction of time spent in each command state),
//! detects temporal anomalies, and estimates mixing time.

use nalgebra::DMatrix;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single anomaly detected by comparing observed transition probability
/// against the stationary distribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// The command that triggered the anomaly.
    pub command: String,
    /// Observed transition probability at the time of detection.
    pub observed_prob: f64,
    /// Expected probability from the stationary distribution.
    pub expected_prob: f64,
    /// How many standard deviations away from expected (z-score proxy).
    pub deviation: f64,
    /// Timestamp (unix epoch seconds) when the anomaly was observed.
    pub timestamp_secs: u64,
}

/// Persistent Markov chain for command transitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMarkovChain {
    /// Maps command names to dense matrix indices.
    command_index: HashMap<String, usize>,
    /// Reverse lookup: index → command name.
    index_command: Vec<String>,
    /// Raw transition counts: `counts[i][j]` = number of times command `i`
    /// was followed by command `j`. Stored as a flat row-major Vec for
    /// serde compatibility.
    counts: Vec<u64>,
    /// Total transitions observed.
    total_transitions: u64,
    /// Maximum matrix dimension (pre-allocates up to this size).
    max_commands: usize,
    /// Cached stationary distribution (invalidated on new observations).
    cached_stationary: Option<Vec<f64>>,
    /// Mixing time estimate (number of steps to reach ε-close to stationary).
    mixing_time_estimate: Option<usize>,
}

impl CommandMarkovChain {
    const DEFAULT_MAX: usize = 512;

    /// Create a new chain with the default maximum number of tracked commands.
    pub fn new() -> Self {
        Self::with_max_commands(Self::DEFAULT_MAX)
    }

    /// Create a new chain with a custom maximum number of commands.
    pub fn with_max_commands(max_commands: usize) -> Self {
        Self {
            command_index: HashMap::new(),
            index_command: Vec::new(),
            counts: vec![0u64; max_commands * max_commands],
            total_transitions: 0,
            max_commands,
            cached_stationary: None,
            mixing_time_estimate: None,
        }
    }

    /// Number of distinct commands currently tracked.
    pub fn num_commands(&self) -> usize {
        self.index_command.len()
    }

    /// Total number of transitions recorded.
    pub fn total_transitions(&self) -> u64 {
        self.total_transitions
    }

    /// Get or create an index for a command name.
    fn ensure_index(&mut self, command: &str) -> usize {
        if let Some(&idx) = self.command_index.get(command) {
            return idx;
        }
        let idx = self.index_command.len();
        assert!(
            idx < self.max_commands,
            "command markov chain: exceeded max_commands ({})",
            self.max_commands
        );
        self.command_index.insert(command.to_string(), idx);
        self.index_command.push(command.to_string());
        idx
    }

    /// Record a transition from `prev` command to `next` command.
    /// Pass `None` for `prev` if this is the first command in a session.
    pub fn record_transition(&mut self, prev: Option<&str>, next: &str) {
        if let Some(p) = prev {
            let i = self.ensure_index(p);
            let j = self.ensure_index(next);
            self.counts[i * self.max_commands + j] += 1;
            self.total_transitions += 1;
            self.cached_stationary = None;
            self.mixing_time_estimate = None;
        } else {
            // First command: just ensure it exists in the index.
            self.ensure_index(next);
        }
    }

    /// Record a batch of transitions from an ordered command sequence.
    pub fn record_sequence(&mut self, commands: &[&str]) {
        if commands.is_empty() {
            return;
        }
        self.record_transition(None, commands[0]);
        for window in commands.windows(2) {
            self.record_transition(Some(window[0]), window[1]);
        }
    }

    /// Build the row-stochastic transition matrix P where P[i][j] is the
    /// probability of transitioning from command i to command j.
    fn build_transition_matrix(&self) -> DMatrix<f64> {
        let n = self.num_commands();
        if n == 0 {
            return DMatrix::zeros(0, 0);
        }
        let mut mat = DMatrix::zeros(n, n);
        for i in 0..n {
            let row_sum: u64 = (0..n).map(|j| self.counts[i * self.max_commands + j]).sum();
            if row_sum == 0 {
                // Dead state: uniform distribution over all states.
                for j in 0..n {
                    mat[(i, j)] = 1.0 / n as f64;
                }
            } else {
                for j in 0..n {
                    mat[(i, j)] = self.counts[i * self.max_commands + j] as f64 / row_sum as f64;
                }
            }
        }
        mat
    }

    /// Compute the stationary distribution π where πP = π.
    ///
    /// Uses the power method on the transpose: iterate π^{t+1} = π^t P^T
    /// until convergence. Falls back to uniform distribution if the chain
    /// has fewer than 2 states.
    pub fn stationary_distribution(&mut self) -> Vec<f64> {
        if let Some(ref cached) = self.cached_stationary {
            return cached.clone();
        }

        let n = self.num_commands();
        if n <= 1 {
            let result = if n == 1 {
                vec![1.0]
            } else {
                vec![]
            };
            self.cached_stationary = Some(result.clone());
            return result;
        }

        let p = self.build_transition_matrix();
        let pt = p.transpose();

        // Power method with 1000 iterations and ε = 1e-12 convergence.
        let mut pi = vec![1.0 / n as f64; n];
        for _ in 0..1000 {
            let pi_vec = nalgebra::DVector::from_vec(pi.clone());
            let pi_next = &pt * &pi_vec;
            let pi_next = pi_next.iter().cloned().collect::<Vec<_>>();
            let diff: f64 = pi
                .iter()
                .zip(pi_next.iter())
                .map(|(a, b)| (a - b).abs())
                .sum();
            pi = pi_next;
            if diff < 1e-12 {
                break;
            }
        }

        // Normalize to sum to 1.
        let sum: f64 = pi.iter().sum();
        if sum > 0.0 {
            for v in pi.iter_mut() {
                *v /= sum;
            }
        }

        self.cached_stationary = Some(pi.clone());
        pi
    }

    /// Return the stationary probability of a given command, or 0.0 if unknown.
    pub fn stationary_prob(&mut self, command: &str) -> f64 {
        let idx = match self.command_index.get(command) {
            Some(&i) => i,
            None => return 0.0,
        };
        let dist = self.stationary_distribution();
        if idx < dist.len() {
            dist[idx]
        } else {
            0.0
        }
    }

    /// Estimate the mixing time: the number of steps until the chain is
    /// within ε of the stationary distribution, starting from the worst-case
    /// initial state.
    ///
    /// Uses total variation distance cutoff ε = 0.01.
    pub fn mixing_time(&mut self) -> usize {
        if let Some(mt) = self.mixing_time_estimate {
            return mt;
        }

        let n = self.num_commands();
        if n <= 1 {
            self.mixing_time_estimate = Some(0);
            return 0;
        }

        let p = self.build_transition_matrix();
        let pi = self.stationary_distribution();
        let pi_dvector = nalgebra::DVector::from_vec(pi.clone());
        let epsilon = 0.01;

        // Run P^t from each unit vector until TV distance < ε.
        let mut worst_steps = 0usize;
        for start in 0..n {
            let mut dist = nalgebra::DVector::zeros(n);
            dist[start] = 1.0;

            for step in 0..10_000 {
                dist = &p * &dist;
                // Total variation distance: 0.5 * Σ |μ(i) - π(i)|
                let tv: f64 = dist
                    .iter()
                    .zip(pi_dvector.iter())
                    .map(|(a, b)| (a - b).abs())
                    .sum::<f64>()
                    * 0.5;
                if tv < epsilon {
                    if step + 1 > worst_steps {
                        worst_steps = step + 1;
                    }
                    break;
                }
            }
        }

        self.mixing_time_estimate = Some(worst_steps);
        worst_steps
    }

    /// Check whether observing `command` at the given timestamp is anomalous.
    ///
    /// Returns `Some(Anomaly)` if the command's stationary probability is
    /// below `threshold` and it was observed anyway.
    pub fn check_anomaly(
        &mut self,
        command: &str,
        timestamp_secs: u64,
        threshold: f64,
    ) -> Option<Anomaly> {
        let expected = self.stationary_prob(command);
        if expected < threshold && self.total_transitions > 20 {
            // Use a heuristic deviation: if expected is near-zero,
            // even one occurrence is a large z-score.
            let n = self.total_transitions.max(1) as f64;
            let std_dev = (expected * (1.0 - expected) / n).sqrt().max(1e-10);
            let observed_prob = 1.0 / n; // single observation frequency
            let deviation = (observed_prob - expected) / std_dev;

            Some(Anomaly {
                command: command.to_string(),
                observed_prob,
                expected_prob: expected,
                deviation,
                timestamp_secs,
            })
        } else {
            None
        }
    }

    /// Get the transition count from command `from` to command `to`.
    pub fn transition_count(&self, from: &str, to: &str) -> u64 {
        let i = match self.command_index.get(from) {
            Some(&idx) => idx,
            None => return 0,
        };
        let j = match self.command_index.get(to) {
            Some(&idx) => idx,
            None => return 0,
        };
        self.counts[i * self.max_commands + j]
    }

    /// Serialize the chain to a JSON string.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// Deserialize a chain from a JSON string.
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

impl Default for CommandMarkovChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_chain_is_empty() {
        let chain = CommandMarkovChain::new();
        assert_eq!(chain.num_commands(), 0);
        assert_eq!(chain.total_transitions(), 0);
    }

    #[test]
    fn record_first_command() {
        let mut chain = CommandMarkovChain::new();
        chain.record_transition(None, "git status");
        assert_eq!(chain.num_commands(), 1);
        assert_eq!(chain.total_transitions(), 0); // no transition yet
    }

    #[test]
    fn record_transition_increments() {
        let mut chain = CommandMarkovChain::new();
        chain.record_transition(Some("git status"), "git add");
        assert_eq!(chain.num_commands(), 2);
        assert_eq!(chain.total_transitions(), 1);
        assert_eq!(chain.transition_count("git status", "git add"), 1);
    }

    #[test]
    fn record_sequence_batch() {
        let mut chain = CommandMarkovChain::new();
        chain.record_sequence(&["git status", "git add", "git commit"]);
        assert_eq!(chain.num_commands(), 3);
        assert_eq!(chain.total_transitions(), 2);
        assert_eq!(chain.transition_count("git status", "git add"), 1);
        assert_eq!(chain.transition_count("git add", "git commit"), 1);
    }

    #[test]
    fn stationary_distribution_deterministic() {
        let mut chain = CommandMarkovChain::new();
        // Create a 2-state chain: A -> B -> A -> B (uniform stationary).
        for _ in 0..100 {
            chain.record_transition(Some("A"), "B");
            chain.record_transition(Some("B"), "A");
        }
        let dist = chain.stationary_distribution();
        assert_eq!(dist.len(), 2);
        assert!((dist[0] - 0.5).abs() < 0.01);
        assert!((dist[1] - 0.5).abs() < 0.01);
    }

    #[test]
    fn stationary_distribution_absorbing_state() {
        let mut chain = CommandMarkovChain::new();
        // A -> A always (absorbing), B -> A sometimes.
        for _ in 0..50 {
            chain.record_transition(Some("A"), "A");
            chain.record_transition(Some("B"), "A");
        }
        let dist = chain.stationary_distribution();
        let a_idx = chain.command_index["A"];
        assert!(dist[a_idx] > 0.9, "A should dominate: got {}", dist[a_idx]);
    }

    #[test]
    fn mixing_time_deterministic_cycle() {
        let mut chain = CommandMarkovChain::new();
        // Deterministic cycle: A -> B -> A -> B. Mixing is instant.
        for _ in 0..100 {
            chain.record_transition(Some("A"), "B");
            chain.record_transition(Some("B"), "A");
        }
        let mt = chain.mixing_time();
        assert!(mt <= 20, "deterministic 2-cycle should mix fast: got {mt}");
    }

    #[test]
    fn mixing_time_is_cached() {
        let mut chain = CommandMarkovChain::new();
        chain.record_sequence(&["x", "y", "z"]);
        let mt1 = chain.mixing_time();
        let mt2 = chain.mixing_time();
        assert_eq!(mt1, mt2);
    }

    #[test]
    fn anomaly_detection_rare_command() {
        let mut chain = CommandMarkovChain::new();
        // Create a chain where "git push" is very rare.
        for _ in 0..100 {
            chain.record_transition(Some("git status"), "git add");
            chain.record_transition(Some("git add"), "git commit");
            chain.record_transition(Some("git commit"), "git status");
        }
        // One rare push at 3am
        chain.record_transition(Some("git status"), "git push");
        let anomaly = chain.check_anomaly("git push", 1685400000, 0.05);
        assert!(anomaly.is_some(), "git push should be anomalous");
        let a = anomaly.unwrap();
        assert_eq!(a.command, "git push");
        assert!(a.deviation > 0.0);
    }

    #[test]
    fn no_anomaly_for_frequent_command() {
        let mut chain = CommandMarkovChain::new();
        for _ in 0..200 {
            chain.record_transition(Some("ls"), "cd");
            chain.record_transition(Some("cd"), "ls");
        }
        let anomaly = chain.check_anomaly("ls", 1685400000, 0.01);
        assert!(anomaly.is_none(), "ls is frequent, should not be anomalous");
    }

    #[test]
    fn serialization_roundtrip() {
        let mut chain = CommandMarkovChain::new();
        chain.record_sequence(&["cargo build", "cargo test", "cargo run"]);
        let json = chain.to_json().unwrap();
        let restored = CommandMarkovChain::from_json(&json).unwrap();
        assert_eq!(restored.num_commands(), chain.num_commands());
        assert_eq!(restored.total_transitions(), chain.total_transitions());
        assert_eq!(
            restored.transition_count("cargo build", "cargo test"),
            1
        );
    }

    #[test]
    fn empty_sequence_is_noop() {
        let mut chain = CommandMarkovChain::new();
        chain.record_sequence(&[]);
        assert_eq!(chain.num_commands(), 0);
    }

    #[test]
    fn single_command_sequence() {
        let mut chain = CommandMarkovChain::new();
        chain.record_sequence(&["lonely"]);
        assert_eq!(chain.num_commands(), 1);
        assert_eq!(chain.total_transitions(), 0);
    }

    #[test]
    fn max_commands_limit_respected() {
        let mut chain = CommandMarkovChain::with_max_commands(4);
        chain.record_sequence(&["a", "b", "c", "d"]);
        assert_eq!(chain.num_commands(), 4);
    }

    #[test]
    #[should_panic(expected = "exceeded max_commands")]
    fn exceeding_max_commands_panics() {
        let mut chain = CommandMarkovChain::with_max_commands(2);
        chain.record_sequence(&["a", "b", "c"]);
    }

    #[test]
    fn unknown_command_stationary_prob_is_zero() {
        let mut chain = CommandMarkovChain::new();
        chain.record_sequence(&["a", "b"]);
        assert_eq!(chain.stationary_prob("nonexistent"), 0.0);
    }
}
