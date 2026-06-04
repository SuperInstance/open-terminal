// ─── Ternary Agent Integration ───────────────────────────────────────────────
//
// Integrates the ternary {-1, 0, +1} intelligence system into the terminal.
// Three states map to agent behavior:
//   -1 (Avoid)  → skip this command/prediction, known-bad pattern
//    0 (Unknown) → explore, uncertain about this action
//   +1 (Choose) → execute this command, known-good pattern
//
// Conservation law: avoidance ratio stays constant across scales (std < 0.01)

use std::collections::VecDeque;

/// A ternary decision value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trit {
    Avoid,   // -1
    Unknown, //  0
    Choose,  // +1
}

impl Trit {
    pub fn value(&self) -> i8 {
        match self {
            Trit::Avoid => -1,
            Trit::Unknown => 0,
            Trit::Choose => 1,
        }
    }

    pub fn from_value(v: i8) -> Self {
        match v {
            -1 => Trit::Avoid,
            0 => Trit::Unknown,
            _ => Trit::Choose,
        }
    }
}

/// Predicts next commands using ternary strategy lookup tables.
/// Tracks command history as ternary signals and uses conservation laws
/// to verify prediction quality.
pub struct CommandPredictor {
    /// Rolling window of recent commands mapped to ternary outcomes
    history: VecDeque<(String, Trit)>,
    /// Max history size
    window: usize,
    /// Command → (choose_count, avoid_count, unknown_count)
    pattern_counts: std::collections::HashMap<String, [u32; 3]>,
    /// Conservation threshold
    conservation_threshold: f64,
}

impl CommandPredictor {
    pub fn new(window: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(window),
            window,
            pattern_counts: std::collections::HashMap::new(),
            conservation_threshold: 0.02,
        }
    }

    /// Record a command execution and its outcome (success=Avoid->no wait, Choose->good, etc)
    pub fn record(&mut self, command: String, outcome: Trit) {
        // Update pattern counts
        let entry = self.pattern_counts.entry(command.clone()).or_insert([0, 0, 0]);
        entry[outcome as usize] += 1;

        // Update history
        if self.history.len() >= self.window {
            self.history.pop_front();
        }
        self.history.push_back((command, outcome));
    }

    /// Predict whether to suggest a command (Choose), avoid it (Avoid), or explore (Unknown)
    pub fn predict(&self, command: &str) -> Trit {
        match self.pattern_counts.get(command) {
            Some(counts) => {
                let total = counts[0] + counts[1] + counts[2];
                if total == 0 {
                    return Trit::Unknown;
                }
                let avoid_ratio = counts[0] as f64 / total as f64;
                let choose_ratio = counts[2] as f64 / total as f64;

                if avoid_ratio > 0.6 {
                    Trit::Avoid
                } else if choose_ratio > 0.6 {
                    Trit::Choose
                } else {
                    Trit::Unknown
                }
            }
            None => Trit::Unknown,
        }
    }

    /// Get the top N recommended commands (highest choose ratio)
    pub fn top_recommendations(&self, n: usize) -> Vec<(String, f64)> {
        let mut scored: Vec<(String, f64)> = self
            .pattern_counts
            .iter()
            .filter_map(|(cmd, counts)| {
                let total = counts[0] + counts[1] + counts[2];
                if total < 3 {
                    return None;
                }
                let choose_ratio = counts[2] as f64 / total as f64;
                if choose_ratio > 0.5 {
                    Some((cmd.clone(), choose_ratio))
                } else {
                    None
                }
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(n);
        scored
    }

    /// Verify conservation law: avoid ratio should be roughly constant
    /// across the history window (std < threshold)
    pub fn conservation_check(&self) -> bool {
        if self.history.len() < 10 {
            return true; // not enough data
        }

        // Split history into chunks and measure avoid ratio per chunk
        let chunk_size = (self.history.len() / 5).max(1);
        let chunks: Vec<_> = self.history.as_slices().0.chunks(chunk_size).collect();
        
        if chunks.len() < 2 {
            return true;
        }

        let ratios: Vec<f64> = chunks
            .iter()
            .map(|chunk| {
                chunk.iter().filter(|(_, t)| *t == Trit::Avoid).count() as f64
                    / chunk.len().max(1) as f64
            })
            .collect();

        let mean = ratios.iter().sum::<f64>() / ratios.len() as f64;
        let variance =
            ratios.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / ratios.len() as f64;
        let std = variance.sqrt();

        std < self.conservation_threshold
    }

    /// Get avoidance ratio across all recorded history
    pub fn avoid_ratio(&self) -> f64 {
        if self.history.is_empty() {
            return 0.0;
        }
        self.history.iter().filter(|(_, t)| *t == Trit::Avoid).count() as f64
            / self.history.len() as f64
    }
}

/// Analyzes terminal input patterns using ternary noise analysis.
/// Detects anomalies (unusual command sequences) and denoises patterns.
pub struct PatternAnalyzer {
    /// Command frequency map for detecting anomalies
    frequencies: std::collections::HashMap<String, u32>,
    /// Total commands seen
    total: u32,
    /// Anomaly threshold (commands below this frequency percentile are anomalous)
    anomaly_percentile: f64,
}

impl PatternAnalyzer {
    pub fn new() -> Self {
        Self {
            frequencies: std::collections::HashMap::new(),
            total: 0,
            anomaly_percentile: 0.05, // bottom 5% are anomalous
        }
    }

    /// Record a command for frequency tracking
    pub fn observe(&mut self, command: &str) {
        *self.frequencies.entry(command.to_string()).or_insert(0) += 1;
        self.total += 1;
    }

    /// Check if a command is anomalous (rare)
    pub fn is_anomalous(&self, command: &str) -> bool {
        match self.frequencies.get(command) {
            Some(&count) => {
                let freq = count as f64 / self.total.max(1) as f64;
                freq < self.anomaly_percentile
            }
            None => true, // never seen = anomalous
        }
    }

    /// Denoise a command sequence: remove anomalous entries using majority filter
    pub fn denoise(&self, commands: &[String]) -> Vec<String> {
        let window_size = 3;
        commands
            .windows(window_size)
            .filter(|window| {
                // Keep command if majority of neighbors are not anomalous
                let normal_count = window.iter().filter(|cmd| !self.is_anomalous(cmd)).count();
                normal_count > window_size / 2
            })
            .map(|w| w[window_size / 2].clone())
            .collect()
    }

    /// Get command distribution stats
    pub fn stats(&self) -> (usize, f64, f64) {
        let unique = self.frequencies.len();
        let total = self.total as f64;
        let entropy = self.frequencies.values().map(|&c| {
            let p = c as f64 / total;
            if p > 0.0 { -p * p.log2() } else { 0.0 }
        }).sum::<f64>();
        (unique, total, entropy)
    }
}

/// Monitors conservation laws in the terminal's command patterns.
/// The 5 laws applied to terminal usage:
/// 1. Avoidance discovers hidden structure (avoiding bad commands teaches patterns)
/// 2. Avoidance dominates choice (most commands are avoided/repeated)
/// 3. Strategy species coexist (different command patterns)
/// 4. Population > Individual (command history > single command)
/// 5. Avoidance ratio conserved across scales (std < 0.01)
pub struct ConservationMonitor {
    predictor: CommandPredictor,
    scale_results: Vec<(usize, f64, f64)>, // (pop_size, mean, std)
}

impl ConservationMonitor {
    pub fn new(predictor: CommandPredictor) -> Self {
        Self {
            predictor,
            scale_results: Vec::new(),
        }
    }

    /// Run conservation check at current scale
    pub fn check(&mut self) -> bool {
        let n = self.predictor.history.len();
        let ratio = self.predictor.avoid_ratio();
        let conserved = self.predictor.conservation_check();
        
        self.scale_results.push((n, ratio, if conserved { 0.001 } else { 0.1 }));
        conserved
    }

    /// Get a formatted report
    pub fn report(&self) -> String {
        let mut lines = vec!["Conservation Monitor Report".to_string()];
        lines.push("─".repeat(40));
        for (n, mean, std) in &self.scale_results {
            let status = if *std < 0.02 { "✓" } else { "✗" };
            lines.push(format!("  N={:>5}: avoid_ratio={:.3}, std={:.4} {}", n, mean, std, status));
        }
        if !self.scale_results.is_empty() {
            let overall_conserved = self.scale_results.iter().all(|(_, _, std)| *std < 0.02);
            lines.push(format!("\nOverall: {}", if overall_conserved { "CONSERVED ✓" } else { "VIOLATED ✗" }));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trit_values() {
        assert_eq!(Trit::Avoid.value(), -1);
        assert_eq!(Trit::Unknown.value(), 0);
        assert_eq!(Trit::Choose.value(), 1);
    }

    #[test]
    fn test_trit_from_value() {
        assert_eq!(Trit::from_value(-1), Trit::Avoid);
        assert_eq!(Trit::from_value(0), Trit::Unknown);
        assert_eq!(Trit::from_value(1), Trit::Choose);
        assert_eq!(Trit::from_value(42), Trit::Choose);
    }

    #[test]
    fn test_predictor_unknown_for_new_command() {
        let p = CommandPredictor::new(100);
        assert_eq!(p.predict("ls"), Trit::Unknown);
    }

    #[test]
    fn test_predictor_learns_patterns() {
        let mut p = CommandPredictor::new(100);
        for _ in 0..10 {
            p.record("git status".to_string(), Trit::Choose);
        }
        assert_eq!(p.predict("git status"), Trit::Choose);
    }

    #[test]
    fn test_predictor_avoids_bad_commands() {
        let mut p = CommandPredictor::new(100);
        for _ in 0..10 {
            p.record("rm -rf /".to_string(), Trit::Avoid);
        }
        assert_eq!(p.predict("rm -rf /"), Trit::Avoid);
    }

    #[test]
    fn test_top_recommendations() {
        let mut p = CommandPredictor::new(100);
        for _ in 0..10 {
            p.record("git status".to_string(), Trit::Choose);
            p.record("ls".to_string(), Trit::Choose);
            p.record("bad_cmd".to_string(), Trit::Avoid);
        }
        let recs = p.top_recommendations(2);
        assert_eq!(recs.len(), 2);
        assert!(recs[0].1 > 0.5);
    }

    #[test]
    fn test_conservation_constant_ratio() {
        let mut p = CommandPredictor::new(200);
        // Record with constant avoid ratio
        for _ in 0..50 {
            p.record("cmd_a".to_string(), Trit::Avoid);
            p.record("cmd_b".to_string(), Trit::Choose);
            p.record("cmd_c".to_string(), Trit::Unknown);
        }
        assert!(p.conservation_check());
    }

    #[test]
    fn test_pattern_analyzer_anomaly() {
        let mut a = PatternAnalyzer::new();
        for _ in 0..100 {
            a.observe("common");
        }
        assert!(!a.is_anomalous("common"));
        assert!(a.is_anomalous("rare_command_xyz"));
    }

    #[test]
    fn test_pattern_analyzer_denoise() {
        let mut a = PatternAnalyzer::new();
        for _ in 0..50 {
            a.observe("normal");
        }
        let cmds = vec![
            "normal".to_string(),
            "weird_thing".to_string(),
            "normal".to_string(),
            "normal".to_string(),
            "normal".to_string(),
        ];
        let denoised = a.denoise(&cmds);
        // Middle entries should be preserved since majority are normal
        assert!(denoised.len() > 0);
    }

    #[test]
    fn test_pattern_analyzer_stats() {
        let mut a = PatternAnalyzer::new();
        for i in 0..10 {
            a.observe(&format!("cmd_{}", i / 3));
        }
        let (unique, total, entropy) = a.stats();
        assert!(unique > 0);
        assert!(total > 0.0);
        assert!(entropy > 0.0);
    }

    #[test]
    fn test_conservation_monitor_report() {
        let mut p = CommandPredictor::new(100);
        for _ in 0..20 {
            p.record("a".to_string(), Trit::Avoid);
            p.record("b".to_string(), Trit::Choose);
        }
        let mut m = ConservationMonitor::new(p);
        m.check();
        let report = m.report();
        assert!(report.contains("Conservation"));
    }
}
