//! Block-spin transformation for command sequences.
//!
//! The renormalization group works by coarse-graining: group neighboring
//! elements ("spins") into blocks, then replace each block with a single
//! "representative" that captures the block's essential character.
//!
//! For command histories:
//! - Block = group of k consecutive commands
//! - Representative = the most frequent command in the block
//! - Apply repeatedly to get the renormalization flow
//!
//! Commands that survive coarse-graining at every scale are the ones
//! that truly define the user's workflow. Ephemeral commands (typos,
//! one-off explorations) get washed out.

/// Block size for coarse-graining.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockSize {
    /// Block of 2 commands.
    B2,
    /// Block of 5 commands.
    B5,
    /// Block of 10 commands.
    B10,
}

impl BlockSize {
    /// The numeric block size.
    pub fn value(&self) -> usize {
        match self {
            Self::B2 => 2,
            Self::B5 => 5,
            Self::B10 => 10,
        }
    }
}

/// A single level of coarse-graining.
#[derive(Debug, Clone, PartialEq)]
pub struct CoarseGrainLevel {
    /// The level index (0 = raw signal, 1 = first coarse-graining, etc.).
    pub level: usize,
    /// The block size used to produce this level.
    pub block_size: usize,
    /// The coarse-grained command sequence.
    pub commands: Vec<String>,
    /// Distribution of commands at this level (command → frequency).
    pub distribution: std::collections::HashMap<String, usize>,
}

impl CoarseGrainLevel {
    fn new(level: usize, block_size: usize, commands: Vec<String>) -> Self {
        let mut distribution = std::collections::HashMap::new();
        for cmd in &commands {
            *distribution.entry(cmd.clone()).or_insert(0) += 1;
        }
        Self {
            level,
            block_size,
            commands,
            distribution,
        }
    }

    /// Number of commands at this level.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether this level is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Shannon entropy of the command distribution at this level.
    pub fn entropy(&self) -> f64 {
        if self.commands.is_empty() {
            return 0.0;
        }
        let total = self.commands.len() as f64;
        let mut h = 0.0;
        for &count in self.distribution.values() {
            if count == 0 {
                continue;
            }
            let p = count as f64 / total;
            h -= p * p.log2();
        }
        h
    }

    /// The most frequent command at this level.
    pub fn dominant_command(&self) -> Option<&str> {
        self.distribution
            .iter()
            .max_by_key(|(_, &count)| count)
            .map(|(cmd, _)| cmd.as_str())
    }
}

/// Block-spin coarse-graining engine.
#[derive(Debug, Clone)]
pub struct CoarseGrainer {
    block_sizes: Vec<BlockSize>,
}

impl CoarseGrainer {
    /// Create a new coarse-grainer with the given block sizes.
    ///
    /// Block sizes are cycled through for successive levels.
    pub fn new(block_sizes: &[BlockSize]) -> Self {
        Self {
            block_sizes: block_sizes.to_vec(),
        }
    }

    /// Create with default block sizes [2, 5, 10, 2, 5, 10, ...].
    pub fn default_grainer() -> Self {
        Self::new(&[BlockSize::B2, BlockSize::B5, BlockSize::B10])
    }

    /// Run the full renormalization flow, producing all levels.
    ///
    /// Returns a vector of levels from raw (level 0) through however many
    /// levels it takes until the signal has fewer than 2 elements or
    /// max_levels is reached.
    pub fn renormalize(&self, commands: &[String], max_levels: usize) -> Vec<CoarseGrainLevel> {
        let mut levels = Vec::new();

        if commands.is_empty() {
            return levels;
        }

        // Level 0: the raw signal
        levels.push(CoarseGrainLevel::new(0, 1, commands.to_vec()));

        let mut current = commands.to_vec();

        for level_idx in 1..=max_levels {
            if current.len() < 2 {
                break;
            }

            let block_size = self.block_size_for_level(level_idx);
            let coarse = self.coarse_grain_once(&current, block_size);

            if coarse.is_empty() {
                break;
            }

            levels.push(CoarseGrainLevel::new(level_idx, block_size, coarse.clone()));
            current = coarse;
        }

        levels
    }

    /// Single coarse-graining step: group into blocks, find representatives.
    fn coarse_grain_once(&self, commands: &[String], block_size: usize) -> Vec<String> {
        if block_size == 0 || commands.is_empty() {
            return Vec::new();
        }

        let mut result = Vec::new();
        let mut i = 0;

        while i < commands.len() {
            let end = (i + block_size).min(commands.len());
            let block = &commands[i..end];
            let representative = self.block_representative(block);
            result.push(representative);
            i += block_size;
        }

        result
    }

    /// Find the representative command for a block.
    ///
    /// The representative is the most frequent command in the block.
    /// Ties are broken by first occurrence.
    fn block_representative(&self, block: &[String]) -> String {
        if block.is_empty() {
            return String::new();
        }

        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        let mut first_seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

        for (i, cmd) in block.iter().enumerate() {
            *counts.entry(cmd.as_str()).or_insert(0) += 1;
            if !first_seen.contains_key(cmd.as_str()) {
                first_seen.insert(cmd.as_str(), i);
            }
        }

        // Most frequent, ties broken by first occurrence
        counts
            .into_iter()
            .max_by(|(cmd_a, count_a), (cmd_b, count_b)| {
                count_a
                    .cmp(count_b)
                    .then_with(|| first_seen[*cmd_a].cmp(&first_seen[*cmd_b]))
            })
            .map(|(cmd, _)| cmd.to_string())
            .unwrap_or_else(|| block[0].clone())
    }

    /// Get the block size to use for a given level.
    fn block_size_for_level(&self, level: usize) -> usize {
        if self.block_sizes.is_empty() {
            return 2;
        }
        let idx = (level - 1) % self.block_sizes.len();
        self.block_sizes[idx].value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_empty_levels() {
        let grainer = CoarseGrainer::default_grainer();
        let levels = grainer.renormalize(&[], 5);
        assert!(levels.is_empty());
    }

    #[test]
    fn single_command_returns_one_level() {
        let grainer = CoarseGrainer::default_grainer();
        let levels = grainer.renormalize(&["git status".to_string()], 5);
        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].commands, vec!["git status"]);
    }

    #[test]
    fn uniform_commands_survive_coarse_graining() {
        let grainer = CoarseGrainer::new(&[BlockSize::B2]);
        let commands = vec!["cargo build".to_string(); 16];
        let levels = grainer.renormalize(&commands, 5);
        // Every level should be all "cargo build"
        for level in &levels {
            assert!(level.commands.iter().all(|c| c == "cargo build"));
        }
    }

    #[test]
    fn alternating_two_commands_survives() {
        let grainer = CoarseGrainer::new(&[BlockSize::B2]);
        let commands: Vec<String> = (0..20)
            .map(|i| if i % 2 == 0 { "a" } else { "b" }.to_string())
            .collect();
        let levels = grainer.renormalize(&commands, 5);
        // With block size 2, blocks are [a,b] → representative is "a" (first seen with tie)
        // Actually both have count 1, tie broken by first occurrence = "a"
        assert!(levels.len() >= 2);
        // Level 1 should be all "a" since every block is [a,b] with tie → first = "a"
        assert!(levels[1].commands.iter().all(|c| c == "a"));
    }

    #[test]
    fn noise_gets_filtered_out() {
        // 90% "git status", 10% noise
        let grainer = CoarseGrainer::new(&[BlockSize::B5]);
        let mut commands = Vec::new();
        for i in 0..100 {
            if i % 10 == 5 {
                commands.push(format!("rare_cmd_{}", i));
            } else {
                commands.push("git status".to_string());
            }
        }
        let levels = grainer.renormalize(&commands, 4);
        // After coarse-graining, rare commands should be mostly gone
        if levels.len() > 2 {
            let last = levels.last().unwrap();
            let git_count = last.commands.iter().filter(|c| **c == "git status").count();
            let total = last.commands.len();
            assert!(git_count as f64 / total as f64 > 0.9);
        }
    }

    #[test]
    fn level_metadata_is_correct() {
        let grainer = CoarseGrainer::new(&[BlockSize::B2, BlockSize::B5]);
        let commands = vec!["a".to_string(); 20];
        let levels = grainer.renormalize(&commands, 3);
        assert_eq!(levels[0].level, 0);
        assert_eq!(levels[0].block_size, 1);
        assert_eq!(levels[1].block_size, 2); // first block size
        assert_eq!(levels[2].block_size, 5); // second block size
    }

    #[test]
    fn entropy_decreases_with_coarse_graining() {
        let grainer = CoarseGrainer::new(&[BlockSize::B5]);
        let mut commands = Vec::new();
        for i in 0..50 {
            commands.push(format!("cmd_{}", i % 5));
        }
        let levels = grainer.renormalize(&commands, 4);
        // Entropy should generally decrease as we coarse-grain
        if levels.len() >= 3 {
            assert!(levels[0].entropy() >= levels.last().unwrap().entropy());
        }
    }

    #[test]
    fn dominant_command_returns_most_frequent() {
        let grainer = CoarseGrainer::default_grainer();
        let commands = vec![
            "a".to_string(),
            "a".to_string(),
            "a".to_string(),
            "b".to_string(),
        ];
        let levels = grainer.renormalize(&commands, 2);
        assert_eq!(levels[0].dominant_command(), Some("a"));
    }

    #[test]
    fn block_representative_tiebreak_first_occurrence() {
        let grainer = CoarseGrainer::new(&[BlockSize::B10]);
        let block = vec!["x".to_string(), "y".to_string()];
        let rep = grainer.block_representative(&block);
        assert_eq!(rep, "x"); // First seen wins ties
    }

    #[test]
    fn three_way_pattern_preserves_majority() {
        // Pattern: a a b a a b a a b ...
        let grainer = CoarseGrainer::new(&[BlockSize::B3]);
        let commands: Vec<String> = (0..30)
            .map(|i| if i % 3 == 2 { "b" } else { "a" }.to_string())
            .collect();
        let levels = grainer.renormalize(&commands, 4);
        // Each block of 3 has 2 a's and 1 b → representative is "a"
        assert!(levels.len() >= 2);
        assert!(levels[1].commands.iter().all(|c| c == "a"));
    }

    #[test]
    fn renormalize_stops_when_signal_too_short() {
        let grainer = CoarseGrainer::new(&[BlockSize::B10]);
        let commands = vec!["a".to_string(); 3];
        let levels = grainer.renormalize(&commands, 10);
        // 3 commands with block size 10 → level 1 has 1 element → stops
        assert!(levels.len() <= 3);
    }

    #[test]
    fn distribution_counts_are_accurate() {
        let grainer = CoarseGrainer::default_grainer();
        let commands = vec![
            "a".to_string(),
            "b".to_string(),
            "a".to_string(),
            "c".to_string(),
        ];
        let levels = grainer.renormalize(&commands, 1);
        assert_eq!(levels[0].distribution.get("a"), Some(&2));
        assert_eq!(levels[0].distribution.get("b"), Some(&1));
        assert_eq!(levels[0].distribution.get("c"), Some(&1));
    }
}
