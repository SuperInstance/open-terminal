//! Next-command prediction using transition probabilities.
//!
//! Given the current command, predict the top-K most likely next commands
//! with confidence scores derived from transition probabilities.
//!
//! The ergodic insight: "After `cargo build`, 73% chance you'll run `cargo test`."
//! The stationary distribution tells us the long-run average; the transition
//! matrix tells us what happens *right now* given what you just did.

use crate::forecast::transition_matrix::TransitionMatrix;

/// A single prediction: the predicted command and its confidence (probability).
#[derive(Debug, Clone, PartialEq)]
pub struct Prediction {
    /// The predicted command.
    pub command: String,
    /// Confidence score: P(next = command | current).
    /// Value in (0, 1] — always positive due to Laplace smoothing.
    pub confidence: f64,
}

impl std::fmt::Display for Prediction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({:.0}%)", self.command, self.confidence * 100.0)
    }
}

/// A complete prediction result for the current command context.
#[derive(Debug, Clone)]
pub struct PredictionResult {
    /// The command that was the input (what the user just ran).
    pub current_command: String,
    /// Top-K predictions sorted by confidence descending.
    pub predictions: Vec<Prediction>,
}

impl PredictionResult {
    /// Format as ghost text suitable for terminal autocomplete display.
    ///
    /// Example: `cargo test  cargo run  cargo clippy`
    /// The first suggestion is the most likely, shown prominently.
    pub fn ghost_text(&self) -> String {
        self.predictions
            .iter()
            .map(|p| p.command.clone())
            .collect::<Vec<_>>()
            .join("  ")
    }

    /// Format as a detailed multi-line explanation.
    ///
    /// Example:
    ///   After `cargo build`:
    ///     → cargo test (73%)
    ///     → cargo run (18%)
    ///     → cargo clippy (5%)
    pub fn detailed(&self) -> String {
        let mut lines = vec![format!("After `{}`:", self.current_command)];
        for p in &self.predictions {
            lines.push(format!("  → {} ({:.0}%)", p.command, p.confidence * 100.0));
        }
        lines.join("\n")
    }
}

/// Predict the next command given the current one.
///
/// Returns the top `k` most likely next commands with their confidence scores.
/// If the current command is unknown to the model, returns an empty result.
pub fn predict_next(matrix: &mut TransitionMatrix, current: &str, k: usize) -> PredictionResult {
    let sorted = matrix.row_sorted(current);
    let predictions: Vec<Prediction> = sorted
        .into_iter()
        .take(k)
        .map(|(command, confidence)| Prediction { command, confidence })
        .collect();

    PredictionResult {
        current_command: current.to_string(),
        predictions,
    }
}

/// Predict the top-3 most likely next commands (default prediction).
pub fn predict_top3(matrix: &mut TransitionMatrix, current: &str) -> PredictionResult {
    predict_next(matrix, current, 3)
}

/// Format a prediction as ghost text for IDE-style autocomplete.
///
/// Returns the most likely completion as ghost text, suitable for rendering
/// in a terminal's input area (dimmed, like IDE autocomplete suggestions).
pub fn ghost_completion(matrix: &mut TransitionMatrix, current: &str) -> Option<String> {
    let result = predict_next(matrix, current, 1);
    result.predictions.first().map(|p| p.command.clone())
}

/// Compute the entropy of the next-command distribution given the current state.
///
/// High entropy = uncertain what comes next (many equally likely options).
/// Low entropy = very predictable next command.
///
/// Entropy H = -Σ p_i log2(p_i)
pub fn prediction_entropy(matrix: &mut TransitionMatrix, current: &str) -> f64 {
    let i = match matrix.index_of(current) {
        Some(idx) => idx,
        None => return 0.0,
    };
    let probs = matrix.probabilities();
    let row = &probs[i];
    let mut entropy = 0.0;
    for &p in row {
        if p > 0.0 {
            entropy -= p * p.log2();
        }
    }
    entropy
}

/// Top-3 prediction with entropy context.
#[derive(Debug, Clone)]
pub struct RichPrediction {
    /// The base prediction result.
    pub result: PredictionResult,
    /// Entropy of the transition distribution from the current command.
    pub entropy_bits: f64,
    /// Maximum possible entropy (log2 of number of states).
    pub max_entropy_bits: f64,
    /// Normalized entropy: entropy / max_entropy ∈ [0, 1].
    /// 0 = deterministic, 1 = uniform (maximally uncertain).
    pub normalized_entropy: f64,
}

/// Produce a rich prediction with entropy metadata.
pub fn predict_rich(matrix: &mut TransitionMatrix, current: &str, k: usize) -> RichPrediction {
    let result = predict_next(matrix, current, k);
    let entropy_bits = prediction_entropy(matrix, current);
    let n = matrix.num_states();
    let max_entropy_bits = if n > 1 { (n as f64).log2() } else { 0.0 };
    let normalized_entropy = if max_entropy_bits > 0.0 {
        entropy_bits / max_entropy_bits
    } else {
        0.0
    };

    RichPrediction {
        result,
        entropy_bits,
        max_entropy_bits,
        normalized_entropy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chain() -> TransitionMatrix {
        let mut m = TransitionMatrix::with_max_states(10);
        // cargo build → cargo test (80%), cargo run (15%), cargo clippy (5%)
        for _ in 0..160 {
            m.record(Some("cargo build"), "cargo test");
        }
        for _ in 0..30 {
            m.record(Some("cargo build"), "cargo run");
        }
        for _ in 0..10 {
            m.record(Some("cargo build"), "cargo clippy");
        }
        m
    }

    #[test]
    fn top3_prediction_orders_by_confidence() {
        let mut m = make_chain();
        let result = predict_top3(&mut m, "cargo build");
        assert_eq!(result.predictions.len(), 3);
        assert_eq!(result.predictions[0].command, "cargo test");
        assert_eq!(result.predictions[1].command, "cargo run");
        assert_eq!(result.predictions[2].command, "cargo clippy");
        // Confidence should be descending
        assert!(result.predictions[0].confidence > result.predictions[1].confidence);
        assert!(result.predictions[1].confidence > result.predictions[2].confidence);
    }

    #[test]
    fn top_prediction_confidence_approximately_correct() {
        let mut m = make_chain();
        let result = predict_top3(&mut m, "cargo build");
        // ~80% for cargo test (with Laplace smoothing it'll be slightly less)
        let conf = result.predictions[0].confidence;
        assert!(conf > 0.75 && conf < 0.85, "expected ~0.80, got {}", conf);
    }

    #[test]
    fn unknown_command_returns_empty() {
        let mut m = make_chain();
        let result = predict_top3(&mut m, "unknown_cmd");
        assert!(result.predictions.is_empty());
    }

    #[test]
    fn ghost_completion_returns_most_likely() {
        let mut m = make_chain();
        let ghost = ghost_completion(&mut m, "cargo build");
        assert_eq!(ghost, Some("cargo test".to_string()));
    }

    #[test]
    fn ghost_completion_unknown_returns_none() {
        let mut m = make_chain();
        let ghost = ghost_completion(&mut m, "nonexistent");
        assert_eq!(ghost, None);
    }

    #[test]
    fn prediction_display_format() {
        let p = Prediction {
            command: "cargo test".to_string(),
            confidence: 0.734,
        };
        assert_eq!(format!("{}", p), "cargo test (73%)");
    }

    #[test]
    fn detailed_format() {
        let mut m = make_chain();
        let result = predict_top3(&mut m, "cargo build");
        let detailed = result.detailed();
        assert!(detailed.starts_with("After `cargo build`:"));
        assert!(detailed.contains("cargo test"));
        assert!(detailed.contains("cargo run"));
    }

    #[test]
    fn ghost_text_format() {
        let mut m = make_chain();
        let result = predict_top3(&mut m, "cargo build");
        let ghost = result.ghost_text();
        assert!(ghost.starts_with("cargo test"));
        assert!(ghost.contains("cargo run"));
    }

    #[test]
    fn predict_next_k_smaller_than_states() {
        let mut m = make_chain();
        let result = predict_next(&mut m, "cargo build", 1);
        assert_eq!(result.predictions.len(), 1);
        assert_eq!(result.predictions[0].command, "cargo test");
    }

    #[test]
    fn prediction_entropy_low_for_deterministic() {
        let mut m = TransitionMatrix::with_max_states(5);
        for _ in 0..1000 {
            m.record(Some("A"), "B");
        }
        let h = prediction_entropy(&mut m, "A");
        // Near-deterministic: entropy should be very close to 0
        assert!(h < 0.1, "entropy for deterministic should be ~0, got {}", h);
    }

    #[test]
    fn prediction_entropy_high_for_uniform() {
        let mut m = TransitionMatrix::with_max_states(4);
        for _ in 0..100 {
            m.record(Some("X"), "A");
            m.record(Some("X"), "B");
            m.record(Some("X"), "C");
            m.record(Some("X"), "D");
        }
        let h = prediction_entropy(&mut m, "X");
        let max_h = (4.0f64).log2(); // 2.0 bits
        assert!(h > max_h * 0.9, "entropy for uniform should be near max, got {} vs {}", h, max_h);
    }

    #[test]
    fn rich_prediction_metadata() {
        let mut m = make_chain();
        let rich = predict_rich(&mut m, "cargo build", 3);
        assert!(rich.entropy_bits > 0.0);
        assert!(rich.max_entropy_bits > 0.0);
        assert!(rich.normalized_entropy >= 0.0 && rich.normalized_entropy <= 1.0);
        assert_eq!(rich.result.predictions.len(), 3);
    }

    #[test]
    fn rich_prediction_unknown_command() {
        let mut m = make_chain();
        let rich = predict_rich(&mut m, "nonexistent", 3);
        assert_eq!(rich.result.predictions.len(), 0);
        assert_eq!(rich.entropy_bits, 0.0);
    }
}
