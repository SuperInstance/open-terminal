//! Command pattern mining and workflow detection.
//!
//! Detects command sequences (not just individual commands):
//! "You always run `cargo build` then `cargo test`" — mines these pairs.
//!
//! Renormalization: group 1000 commands into 10 "workflow patterns".
//! Detect learning plateaus (fixed points in the renormalization flow):
//! "You've been doing the same git workflow for 3 months — here's a script"

use std::collections::HashMap;

/// A detected workflow pattern (sequence of commands that repeats).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowPattern {
    /// The command sequence.
    pub commands: Vec<String>,
    /// How many times this pattern was observed.
    pub frequency: u32,
    /// First occurrence index in the command stream.
    pub first_seen: usize,
    /// Confidence score [0, 1].
    pub confidence: f64,
}

impl WorkflowPattern {
    /// A human-readable label for this pattern.
    pub fn label(&self) -> String {
        self.commands.join(" → ")
    }
}

/// A learning plateau: a period where the user's command patterns stabilize.
#[derive(Debug, Clone)]
pub struct LearningPlateau {
    /// Index range (start, end) of the plateau in the command stream.
    pub range: (usize, usize),
    /// The dominant pattern during this plateau.
    pub dominant_pattern: Vec<String>,
    /// Duration in number of commands.
    pub span: usize,
    /// How stable the pattern is (0 = no plateau, 1 = perfectly fixed).
    pub stability: f64,
}

/// The pattern miner that processes command streams.
#[derive(Debug, Clone)]
pub struct PatternMiner {
    /// Raw command strings in order.
    commands: Vec<String>,
    /// Timestamps for each command.
    timestamps: Vec<u64>,
}

impl PatternMiner {
    /// Create a miner from (command, timestamp) pairs.
    pub fn from_commands(pairs: &[(String, u64)]) -> Self {
        let (commands, timestamps): (Vec<String>, Vec<u64>) = pairs.iter().cloned().unzip();
        Self { commands, timestamps }
    }

    /// Detect workflow patterns of length 2 (command pairs).
    pub fn detect_pairs(&self) -> Vec<WorkflowPattern> {
        self.detect_patterns_of_length(2)
    }

    /// Detect workflow patterns of the given length.
    pub fn detect_patterns_of_length(&self, length: usize) -> Vec<WorkflowPattern> {
        if self.commands.len() < length {
            return Vec::new();
        }

        let mut freq: HashMap<Vec<String>, (u32, usize)> = HashMap::new();
        for i in 0..=self.commands.len() - length {
            let seq: Vec<String> = self.commands[i..i + length].to_vec();
            let entry = freq.entry(seq).or_insert((0, i));
            entry.0 += 1;
        }

        let mut patterns: Vec<WorkflowPattern> = freq
            .into_iter()
            .filter(|(_, (count, _))| *count >= 2)
            .map(|(commands, (frequency, first_seen))| {
                let confidence = (frequency as f64 / (self.commands.len() as f64 - length as f64 + 1.0))
                    .min(1.0);
                WorkflowPattern {
                    commands,
                    frequency,
                    first_seen,
                    confidence,
                }
            })
            .collect();

        patterns.sort_by(|a, b| b.frequency.cmp(&a.frequency));
        patterns
    }

    /// Detect all workflow patterns (length 2 through 5).
    pub fn detect_patterns(&self) -> Vec<WorkflowPattern> {
        let mut all = Vec::new();
        for len in 2..=5.min(self.commands.len()) {
            all.extend(self.detect_patterns_of_length(len));
        }
        all.sort_by(|a, b| b.frequency.cmp(&a.frequency));
        all
    }

    /// Renormalize: group commands into N high-level workflow patterns.
    /// This is a simplified renormalization that takes the top-N most frequent
    /// subsequences and returns them as the "coarse-grained" view.
    pub fn renormalize(&self, n: usize) -> Vec<WorkflowPattern> {
        let all = self.detect_patterns();
        all.into_iter().take(n).collect()
    }

    /// Detect learning plateaus: periods where the same pattern repeats
    /// without significant variation.
    ///
    /// A plateau is detected when a sliding window of commands has high
    /// similarity (same commands in similar order) for an extended period.
    pub fn detect_plateaus(&self) -> Vec<LearningPlateau> {
        if self.commands.len() < 10 {
            return Vec::new();
        }

        let window_size = 10;
        let step = 5;
        let mut plateaus = Vec::new();

        let mut i = 0;
        while i + window_size <= self.commands.len() {
            let window: &[String] = &self.commands[i..i + window_size];
            let uniq: std::collections::HashSet<&String> = window.iter().collect();
            let diversity = uniq.len() as f64 / window_size as f64;

            // Low diversity = plateau (doing the same things)
            if diversity <= 0.4 {
                let end = (i + window_size).min(self.commands.len());
                let dominant = self.dominant_sequence(i, end);
                let stability = 1.0 - diversity;

                plateaus.push(LearningPlateau {
                    range: (i, end),
                    dominant_pattern: dominant,
                    span: end - i,
                    stability,
                });
            }
            i += step;
        }

        // Merge overlapping plateaus
        merge_plateaus(&mut plateaus);
        plateaus
    }

    /// Find the most common subsequence of commands in a range.
    fn dominant_sequence(&self, start: usize, end: usize) -> Vec<String> {
        let mut freq: HashMap<&String, u32> = HashMap::new();
        for cmd in &self.commands[start..end] {
            *freq.entry(cmd).or_insert(0) += 1;
        }
        let mut sorted: Vec<_> = freq.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.into_iter().take(3).map(|(s, _)| s.clone()).collect()
    }

    /// The raw commands.
    pub fn commands(&self) -> &[String] {
        &self.commands
    }
}

/// Merge overlapping or adjacent plateaus.
fn merge_plateaus(plateaus: &mut Vec<LearningPlateau>) {
    if plateaus.len() <= 1 {
        return;
    }
    plateaus.sort_by_key(|p| p.range.0);
    let mut merged = Vec::new();
    let mut current = plateaus[0].clone();
    for p in plateaus.iter().skip(1) {
        if p.range.0 <= current.range.1 {
            // Overlapping or adjacent — merge.
            current.range.1 = current.range.1.max(p.range.1);
            current.span = current.range.1 - current.range.0;
            current.stability = current.stability.max(p.stability);
            if p.dominant_pattern.len() > current.dominant_pattern.len() {
                current.dominant_pattern = p.dominant_pattern.clone();
            }
        } else {
            merged.push(current);
            current = p.clone();
        }
    }
    merged.push(current);
    *plateaus = merged;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pairs(cmds: &[&str]) -> Vec<(String, u64)> {
        cmds.iter()
            .enumerate()
            .map(|(i, c)| (c.to_string(), 1700000000 + i as u64 * 60))
            .collect()
    }

    #[test]
    fn detect_simple_pair() {
        let pairs = make_pairs(&["cargo build", "cargo test", "cargo build", "cargo test"]);
        let miner = PatternMiner::from_commands(&pairs);
        let patterns = miner.detect_pairs();
        assert!(patterns.iter().any(|p| p.commands == vec!["cargo build", "cargo test"]));
    }

    #[test]
    fn detect_triple() {
        let pairs = make_pairs(&[
            "git add", "git commit", "git push",
            "git add", "git commit", "git push",
        ]);
        let miner = PatternMiner::from_commands(&pairs);
        let triples = miner.detect_patterns_of_length(3);
        assert!(triples.iter().any(|p| p.commands == vec!["git add", "git commit", "git push"]));
    }

    #[test]
    fn no_patterns_too_few() {
        let pairs = make_pairs(&["cargo build"]);
        let miner = PatternMiner::from_commands(&pairs);
        assert!(miner.detect_patterns().is_empty());
    }

    #[test]
    fn no_patterns_no_repeats() {
        let pairs = make_pairs(&["a", "b", "c", "d"]);
        let miner = PatternMiner::from_commands(&pairs);
        let patterns = miner.detect_patterns();
        assert!(patterns.is_empty());
    }

    #[test]
    fn renormalize_top_n() {
        let cmds = ["a", "b", "a", "b", "a", "b", "c", "d", "c", "d"];
        let pairs = make_pairs(&cmds);
        let miner = PatternMiner::from_commands(&pairs);
        let top2 = miner.renormalize(2);
        assert!(top2.len() <= 2);
        assert!(!top2.is_empty());
    }

    #[test]
    fn plateau_detection() {
        // 20 identical commands → should trigger plateau
        let cmds: Vec<&str> = (0..20).map(|_| "same").collect();
        let pairs = make_pairs(&cmds);
        let miner = PatternMiner::from_commands(&pairs);
        let plateaus = miner.detect_plateaus();
        assert!(!plateaus.is_empty());
        assert!(plateaus[0].stability > 0.5);
    }

    #[test]
    fn no_plateau_diverse() {
        let cmds: Vec<&str> = (0..20).map(|i| {
            Box::leak(format!("cmd_{}", i).into_boxed_str()) as &str
        }).collect();
        let pairs = make_pairs(&cmds);
        let miner = PatternMiner::from_commands(&pairs);
        let plateaus = miner.detect_plateaus();
        assert!(plateaus.is_empty());
    }

    #[test]
    fn pattern_label() {
        let p = WorkflowPattern {
            commands: vec!["git add".into(), "git commit".into()],
            frequency: 5,
            first_seen: 0,
            confidence: 0.8,
        };
        assert_eq!(p.label(), "git add → git commit");
    }

    #[test]
    fn detect_patterns_multi_length() {
        let cmds = [
            "git add", "git commit", "git push",
            "git add", "git commit", "git push",
        ];
        let pairs = make_pairs(&cmds);
        let miner = PatternMiner::from_commands(&pairs);
        let all = miner.detect_patterns();
        // Should have pairs and triples
        assert!(all.iter().any(|p| p.commands.len() == 2));
        assert!(all.iter().any(|p| p.commands.len() == 3));
    }

    #[test]
    fn plateau_merging() {
        let p1 = LearningPlateau {
            range: (0, 10),
            dominant_pattern: vec!["a".into()],
            span: 10,
            stability: 0.9,
        };
        let p2 = LearningPlateau {
            range: (8, 18),
            dominant_pattern: vec!["a".into(), "b".into()],
            span: 10,
            stability: 0.85,
        };
        let mut plateaus = vec![p1, p2];
        merge_plateaus(&mut plateaus);
        assert_eq!(plateaus.len(), 1);
        assert_eq!(plateaus[0].range, (0, 18));
    }
}
