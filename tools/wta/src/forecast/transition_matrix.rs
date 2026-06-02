//! Transition matrix construction for command Markov chains.
//!
//! Tracks command sequences as a first-order Markov chain:
//! - States = unique commands (up to 100 most common)
//! - Transitions = command→command frequency counts
//! - Laplace smoothing (α=1) for unseen transitions
//! - Row-normalization yields stochastic transition probabilities
//!
//! The resulting transition matrix is the engine of all forecasting:
//!     P(next_command | current_command) = P[current][next]
//!
//! Serialize/deserialize for persistence across sessions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Smoothing constant for Laplace smoothing. Prevents zero-probability
/// transitions and ensures every state has a non-zero chance of reaching
/// every other state (ergodicity guarantee).
const LAPLACE_ALPHA: u64 = 1;

/// Default maximum number of distinct command states to track.
/// Commands beyond the top-N by frequency are collapsed into "__other__".
const DEFAULT_MAX_STATES: usize = 100;

/// A row-stochastic transition matrix built from observed command sequences.
///
/// The matrix `P` is such that `P[i][j]` is the probability of transitioning
/// from command state `i` to command state `j`. Constructed from raw counts
/// with Laplace smoothing, then row-normalized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionMatrix {
    /// Command name → dense index.
    command_to_idx: HashMap<String, usize>,
    /// Dense index → command name.
    idx_to_command: Vec<String>,
    /// Raw transition counts: `counts[i * max_states + j]`.
    counts: Vec<u64>,
    /// Maximum number of states.
    max_states: usize,
    /// Total transitions observed.
    total_transitions: u64,
    /// Cached row-stochastic matrix (lazily computed, invalidated on mutation).
    cached_probabilities: Option<Vec<Vec<f64>>>,
}

impl TransitionMatrix {
    /// Create a new transition matrix with the default 100-state cap.
    pub fn new() -> Self {
        Self::with_max_states(DEFAULT_MAX_STATES)
    }

    /// Create a transition matrix with a custom state cap.
    pub fn with_max_states(max_states: usize) -> Self {
        Self {
            command_to_idx: HashMap::new(),
            idx_to_command: Vec::new(),
            counts: vec![0u64; max_states * max_states],
            max_states,
            total_transitions: 0,
            cached_probabilities: None,
        }
    }

    /// Number of distinct command states currently tracked.
    pub fn num_states(&self) -> usize {
        self.idx_to_command.len()
    }

    /// Total number of transitions recorded.
    pub fn total_transitions(&self) -> u64 {
        self.total_transitions
    }

    /// Get the command name for a state index.
    pub fn command_at(&self, idx: usize) -> Option<&str> {
        self.idx_to_command.get(idx).map(|s| s.as_str())
    }

    /// Get the index for a command name, if it exists.
    pub fn index_of(&self, command: &str) -> Option<usize> {
        self.command_to_idx.get(command).copied()
    }

    /// Ensure a command has an index, allocating one if necessary.
    /// Returns `None` if we've hit the state cap.
    fn ensure_index(&mut self, command: &str) -> Option<usize> {
        if let Some(&idx) = self.command_to_idx.get(command) {
            return Some(idx);
        }
        if self.idx_to_command.len() >= self.max_states {
            return None;
        }
        let idx = self.idx_to_command.len();
        self.command_to_idx.insert(command.to_string(), idx);
        self.idx_to_command.push(command.to_string());
        Some(idx)
    }

    /// Record a single transition from `prev` to `next`.
    /// If `prev` is `None`, only registers `next` as a known state.
    pub fn record(&mut self, prev: Option<&str>, next: &str) {
        if let Some(next_idx) = self.ensure_index(next) {
            if let Some(p) = prev {
                if let Some(prev_idx) = self.ensure_index(p) {
                    self.counts[prev_idx * self.max_states + next_idx] += 1;
                    self.total_transitions += 1;
                    self.cached_probabilities = None;
                }
            }
        }
    }

    /// Record a batch of transitions from an ordered command sequence.
    pub fn record_sequence(&mut self, commands: &[&str]) {
        if commands.is_empty() {
            return;
        }
        self.record(None, commands[0]);
        for window in commands.windows(2) {
            self.record(Some(window[0]), window[1]);
        }
    }

    /// Get the raw transition count from state `i` to state `j`.
    pub fn count(&self, from: usize, to: usize) -> u64 {
        if from >= self.num_states() || to >= self.num_states() {
            0
        } else {
            self.counts[from * self.max_states + to]
        }
    }

    /// Get the raw transition count between two named commands.
    pub fn count_named(&self, from: &str, to: &str) -> u64 {
        let i = self.command_to_idx.get(from)?;
        let j = self.command_to_idx.get(to)?;
        Some(self.counts[i * self.max_states + j])
            .unwrap_or(0)
    }

    /// Compute (or return cached) the row-stochastic transition probabilities.
    ///
    /// Each row is Laplace-smoothed and normalized:
    ///     P[i][j] = (count[i][j] + α) / (Σ_k count[i][k] + α·N)
    ///
    /// This guarantees:
    /// - Every probability > 0 (no absorbing dead ends)
    /// - Each row sums to 1.0
    /// - The chain is ergodic (all states communicate)
    pub fn probabilities(&mut self) -> &[Vec<f64>] {
        if self.cached_probabilities.is_some() {
            return self.cached_probabilities.as_ref().unwrap();
        }

        let n = self.num_states();
        let alpha = LAPLACE_ALPHA as f64;

        let mut probs = Vec::with_capacity(n);
        for i in 0..n {
            let row_sum: u64 = (0..n).map(|j| self.counts[i * self.max_states + j]).sum();
            let denominator = row_sum as f64 + alpha * n as f64;
            let mut row = Vec::with_capacity(n);
            for j in 0..n {
                let smoothed = self.counts[i * self.max_states + j] as f64 + alpha;
                row.push(smoothed / denominator);
            }
            probs.push(row);
        }

        self.cached_probabilities = Some(probs);
        self.cached_probabilities.as_ref().unwrap()
    }

    /// Get the transition probability P(next | current) for named commands.
    /// Returns 0.0 if either command is unknown.
    pub fn transition_prob(&mut self, from: &str, to: &str) -> f64 {
        let i = match self.command_to_idx.get(from) {
            Some(&idx) => idx,
            None => return 0.0,
        };
        let j = match self.command_to_idx.get(to) {
            Some(&idx) => idx,
            None => return 0.0,
        };
        self.probabilities()[i][j]
    }

    /// Get the full probability row for a given command state.
    /// Returns pairs of (command_name, probability), sorted by probability descending.
    pub fn row_sorted(&mut self, command: &str) -> Vec<(String, f64)> {
        let i = match self.command_to_idx.get(command) {
            Some(&idx) => idx,
            None => return vec![],
        };
        let probs = self.probabilities();
        let mut result: Vec<(String, f64)> = (0..self.num_states())
            .map(|j| (self.idx_to_command[j].clone(), probs[i][j]))
            .collect();
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        result
    }

    /// Compute the stationary distribution π where πP = π.
    ///
    /// Uses the power method: iterate π^{t+1} = π^t P until convergence.
    /// The Laplace-smoothed matrix is guaranteed ergodic, so the stationary
    /// distribution exists and is unique.
    pub fn stationary_distribution(&mut self) -> Vec<f64> {
        let n = self.num_states();
        if n == 0 {
            return vec![];
        }
        if n == 1 {
            return vec![1.0];
        }

        let probs = self.probabilities().to_vec();

        // Power method
        let mut pi = vec![1.0 / n as f64; n];
        for _ in 0..2000 {
            let mut pi_next = vec![0.0; n];
            for j in 0..n {
                for i in 0..n {
                    pi_next[j] += pi[i] * probs[i][j];
                }
            }
            let diff: f64 = pi.iter().zip(pi_next.iter()).map(|(a, b)| (a - b).abs()).sum();
            pi = pi_next;
            if diff < 1e-14 {
                break;
            }
        }

        // Normalize
        let sum: f64 = pi.iter().sum();
        if sum > 0.0 {
            for v in pi.iter_mut() {
                *v /= sum;
            }
        }
        pi
    }

    /// Get the stationary probability for a named command.
    pub fn stationary_prob(&mut self, command: &str) -> f64 {
        let i = match self.command_to_idx.get(command) {
            Some(&idx) => idx,
            None => return 0.0,
        };
        let dist = self.stationary_distribution();
        dist.get(i).copied().unwrap_or(0.0)
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

impl Default for TransitionMatrix {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_matrix_is_empty() {
        let m = TransitionMatrix::new();
        assert_eq!(m.num_states(), 0);
        assert_eq!(m.total_transitions(), 0);
    }

    #[test]
    fn record_single_transition() {
        let mut m = TransitionMatrix::new();
        m.record(Some("git status"), "git add");
        assert_eq!(m.num_states(), 2);
        assert_eq!(m.total_transitions(), 1);
        assert_eq!(m.count_named("git status", "git add"), 1);
    }

    #[test]
    fn record_sequence_batch() {
        let mut m = TransitionMatrix::new();
        m.record_sequence(&["a", "b", "c", "a", "b"]);
        assert_eq!(m.num_states(), 3);
        assert_eq!(m.total_transitions(), 4);
        assert_eq!(m.count_named("a", "b"), 2);
        assert_eq!(m.count_named("b", "c"), 1);
        assert_eq!(m.count_named("c", "a"), 1);
    }

    #[test]
    fn empty_sequence_is_noop() {
        let mut m = TransitionMatrix::new();
        m.record_sequence(&[]);
        assert_eq!(m.num_states(), 0);
    }

    #[test]
    fn laplace_smoothing_no_zeros() {
        let mut m = TransitionMatrix::with_max_states(3);
        m.record_sequence(&["x", "y"]);
        // Even unobserved transitions have positive probability.
        let probs = m.probabilities();
        for i in 0..m.num_states() {
            for j in 0..m.num_states() {
                assert!(probs[i][j] > 0.0, "P[{}][{}] = {} should be > 0", i, j, probs[i][j]);
            }
        }
    }

    #[test]
    fn rows_sum_to_one() {
        let mut m = TransitionMatrix::with_max_states(5);
        m.record_sequence(&["a", "b", "c", "a", "c", "b", "a"]);
        let probs = m.probabilities();
        for i in 0..m.num_states() {
            let row_sum: f64 = probs[i].iter().sum();
            assert!((row_sum - 1.0).abs() < 1e-10, "row {} sums to {}", i, row_sum);
        }
    }

    #[test]
    fn transition_prob_deterministic_chain() {
        let mut m = TransitionMatrix::with_max_states(3);
        // A always goes to B (many observations drown out Laplace)
        for _ in 0..1000 {
            m.record(Some("A"), "B");
        }
        let p = m.transition_prob("A", "B");
        assert!(p > 0.99, "A→B should be ~1.0, got {}", p);
    }

    #[test]
    fn transition_prob_unknown_commands() {
        let mut m = TransitionMatrix::new();
        m.record_sequence(&["a", "b"]);
        assert_eq!(m.transition_prob("nonexistent", "a"), 0.0);
        assert_eq!(m.transition_prob("a", "nonexistent"), 0.0);
    }

    #[test]
    fn row_sorted_descending() {
        let mut m = TransitionMatrix::with_max_states(5);
        for _ in 0..100 {
            m.record(Some("x"), "a");
        }
        for _ in 0..10 {
            m.record(Some("x"), "b");
        }
        m.record(Some("x"), "c");
        let sorted = m.row_sorted("x");
        assert_eq!(sorted[0].0, "a");
        assert!(sorted[0].1 > sorted[1].1);
    }

    #[test]
    fn stationary_distribution_uniform_for_cycle() {
        let mut m = TransitionMatrix::with_max_states(3);
        for _ in 0..200 {
            m.record(Some("A"), "B");
            m.record(Some("B"), "C");
            m.record(Some("C"), "A");
        }
        let dist = m.stationary_distribution();
        assert_eq!(dist.len(), 3);
        for p in &dist {
            assert!((p - 1.0 / 3.0).abs() < 0.05, "expected ~0.333, got {}", p);
        }
    }

    #[test]
    fn stationary_distribution_single_state() {
        let mut m = TransitionMatrix::with_max_states(2);
        m.record(None, "only");
        let dist = m.stationary_distribution();
        assert_eq!(dist, vec![1.0]);
    }

    #[test]
    fn stationary_distribution_empty() {
        let mut m = TransitionMatrix::new();
        assert!(m.stationary_distribution().is_empty());
    }

    #[test]
    fn serialization_roundtrip() {
        let mut m = TransitionMatrix::with_max_states(10);
        m.record_sequence(&["cargo build", "cargo test", "cargo run"]);
        let json = m.to_json().unwrap();
        let restored = TransitionMatrix::from_json(&json).unwrap();
        assert_eq!(restored.num_states(), m.num_states());
        assert_eq!(restored.total_transitions(), m.total_transitions());
        assert_eq!(restored.count_named("cargo build", "cargo test"), 1);
    }

    #[test]
    fn max_states_cap_respected() {
        let mut m = TransitionMatrix::with_max_states(3);
        m.record_sequence(&["a", "b", "c"]);
        // 4th command exceeds cap — it should be silently dropped
        m.record(Some("c"), "d");
        assert_eq!(m.num_states(), 3);
    }

    #[test]
    fn probabilities_cached_and_invalidated() {
        let mut m = TransitionMatrix::with_max_states(5);
        m.record_sequence(&["a", "b"]);
        let _ = m.probabilities();
        // Adding new data invalidates cache
        m.record(Some("b"), "c");
        let probs = m.probabilities();
        // b→c should now have a meaningful probability
        let b_idx = m.index_of("b").unwrap();
        let c_idx = m.index_of("c").unwrap();
        assert!(probs[b_idx][c_idx] > 0.0);
    }
}
