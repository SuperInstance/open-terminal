//! Persistence barcode and diagram visualization.
//!
//! Generates persistence diagrams from command history showing which
//! commands "persist" through time vs are ephemeral.
//!
//! ASCII visualization for terminal display:
//! ```text
//! █████░░░░░█████████░░░████░░░░░░░████████
//! ^Rust  ^Node          ^Git        ^Docker
//! ```

use crate::griot_history::decay::{DecayModel, PERSISTENCE_THRESHOLD};

/// A persistence barcode: a visual representation of which commands
/// survive the decay filter over time.
#[derive(Debug, Clone)]
pub struct PersistenceBarcode {
    /// Each slot is (command, strength, persists).
    slots: Vec<BarcodeSlot>,
    /// The reference time used.
    reference_time: u64,
}

/// A single slot in the barcode.
#[derive(Debug, Clone)]
pub struct BarcodeSlot {
    /// The command string.
    pub command: String,
    /// The computed strength.
    pub strength: f64,
    /// Whether this command persists.
    pub persists: bool,
}

/// A persistence diagram showing birth/death of command strengths.
#[derive(Debug, Clone)]
pub struct PersistenceDiagram {
    /// Points: (birth_strength, death_strength).
    /// birth = initial strength, death = strength when it drops below threshold.
    pub points: Vec<(String, f64, f64)>,
}

impl PersistenceBarcode {
    /// Build a barcode from a decay model.
    pub fn from_model(model: &DecayModel) -> Self {
        let slots = model
            .all_strengths()
            .into_iter()
            .map(|(command, strength)| BarcodeSlot {
                command,
                strength: strength.0,
                persists: strength.persists(),
            })
            .collect();

        PersistenceBarcode {
            slots,
            reference_time: model.reference_time(),
        }
    }

    /// Total number of command slots.
    pub fn total_commands(&self) -> usize {
        self.slots.len()
    }

    /// Number of persisting commands.
    pub fn persisting_count(&self) -> usize {
        self.slots.iter().filter(|s| s.persists).count()
    }

    /// Ratio of persisting to total commands.
    pub fn persistence_ratio(&self) -> f64 {
        if self.slots.is_empty() {
            0.0
        } else {
            self.persisting_count() as f64 / self.slots.len() as f64
        }
    }

    /// Render the barcode as ASCII art.
    ///
    /// Uses block characters to show strength levels:
    ///   █ = full strength (≥ 0.8)
    ///   ▓ = strong (≥ 0.6)
    ///   ▒ = medium (≥ 0.3)
    ///   ░ = weak (≥ threshold)
    ///   · = below threshold (ephemeral)
    pub fn render_ascii(&self, width: usize) -> String {
        if self.slots.is_empty() {
            return "(empty barcode)".to_string();
        }

        // Map slots to characters
        let chars: Vec<char> = self.slots.iter().map(|s| strength_char(s.strength, s.persists)).collect();

        // If we have more slots than width, sample evenly.
        let sampled: Vec<char> = if chars.len() <= width {
            chars
        } else {
            let step = chars.len() as f64 / width as f64;
            (0..width)
                .map(|i| {
                    let idx = (i as f64 * step) as usize;
                    chars[idx.min(chars.len() - 1)]
                })
                .collect()
        };

        sampled.into_iter().collect()
    }

    /// Render a labeled barcode showing which command groups persist.
    ///
    /// Returns lines of: barcode + label annotations.
    pub fn render_labeled(&self, width: usize) -> String {
        let barcode = self.render_ascii(width);

        if self.slots.is_empty() {
            return barcode;
        }

        let mut result = barcode.clone();
        result.push('\n');

        // Show top persisting commands
        let mut persisting: Vec<&BarcodeSlot> = self.slots.iter().filter(|s| s.persists).collect();
        persisting.sort_by(|a, b| b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal));

        let top_n = 5.min(persisting.len());
        if top_n > 0 {
            result.push_str("Persisting commands:\n");
            for slot in persisting.iter().take(top_n) {
                result.push_str(&format!("  {} [{:.2}]\n", slot.command, slot.strength));
            }
        }

        let ephemeral_count = self.slots.iter().filter(|s| !s.persists).count();
        if ephemeral_count > 0 {
            result.push_str(&format!("Ephemeral: {} commands below threshold\n", ephemeral_count));
        }

        result
    }

    /// Get all slots.
    pub fn slots(&self) -> &[BarcodeSlot] {
        &self.slots
    }
}

impl PersistenceDiagram {
    /// Build a persistence diagram from a decay model.
    ///
    /// For each unique command, tracks:
    ///   birth_strength: the strength at its most recent occurrence
    ///   death_strength: what its strength would be at the reference time
    ///     if it were never repeated
    pub fn from_model(model: &DecayModel) -> Self {
        use crate::griot_history::decay::CommandRecord;

        let records = model.records();
        let ref_time = model.reference_time();

        // Group by command
        let mut cmd_records: std::collections::HashMap<String, Vec<&CommandRecord>> =
            std::collections::HashMap::new();
        for rec in records {
            cmd_records
                .entry(rec.command.clone())
                .or_default()
                .push(rec);
        }

        let points: Vec<(String, f64, f64)> = cmd_records
            .into_iter()
            .map(|(cmd, recs)| {
                // Birth: strength of the most recent occurrence
                let latest = recs.iter().max_by_key(|r| r.timestamp).unwrap();
                let birth = latest.strength_at(ref_time).0;

                // Death: strength of the oldest occurrence (no retelling boost)
                let oldest = recs.iter().min_by_key(|r| r.timestamp).unwrap();
                let oldest_age = ref_time.saturating_sub(oldest.timestamp) as f64;
                let death = (-super::decay::LAMBDA * oldest_age).exp();

                (cmd, birth, death)
            })
            .collect();

        PersistenceDiagram { points }
    }

    /// Render the diagram as ASCII.
    ///
    /// ```
    /// command        birth  death  gap
    /// cargo build    0.95   0.12   ████████████░░
    /// cargo test     0.88   0.20   ██████████░░░░
    /// ```
    pub fn render_ascii(&self, bar_width: usize) -> String {
        if self.points.is_empty() {
            return "(empty diagram)".to_string();
        }

        let mut result = String::new();
        result.push_str(&format!(
            "{:<20} {:>6} {:>6} {}\n",
            "command", "birth", "death", "persistence"
        ));

        for (cmd, birth, death) in &self.points {
            let gap = birth - death;
            let filled = (gap * bar_width as f64).round() as usize;
            let filled = filled.min(bar_width);
            let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
            result.push_str(&format!(
                "{:<20} {:>5.2}  {:>5.2}  {}\n",
                truncate(cmd, 20),
                birth,
                death,
                bar
            ));
        }

        result
    }

    /// Number of points in the diagram.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether the diagram is empty.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// Map a strength value to a block character.
fn strength_char(strength: f64, persists: bool) -> char {
    if !persists {
        '·'
    } else if strength >= 0.8 {
        '█'
    } else if strength >= 0.6 {
        '▓'
    } else if strength >= 0.3 {
        '▒'
    } else {
        '░'
    }
}

/// Truncate a string to max_len bytes, appending "…" if truncated.
///
/// Uses char-based slicing to avoid panicking on multi-byte UTF-8
/// boundaries (the original byte-index slicing was a crash vector).
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 1 {
        let byte_end = s
            .char_indices()
            .take_while(|(i, c)| *i + c.len_utf8() <= max_len - 1)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        format!("{}…", &s[..byte_end])
    } else {
        "…".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::griot_history::decay::DecayModel;

    fn ts(days_ago: u64) -> u64 {
        1700000000 - days_ago * 86400
    }

    #[test]
    fn empty_barcode() {
        let model = DecayModel::new();
        let barcode = PersistenceBarcode::from_model(&model);
        assert_eq!(barcode.total_commands(), 0);
        assert_eq!(barcode.persisting_count(), 0);
        assert_eq!(barcode.render_ascii(40), "(empty barcode)");
    }

    #[test]
    fn barcode_with_commands() {
        let mut model = DecayModel::new();
        model.record("cargo build".into(), ts(0));
        model.record("ls".into(), ts(30));
        let barcode = PersistenceBarcode::from_model(&model);
        assert_eq!(barcode.total_commands(), 2);
        assert!(barcode.persisting_count() >= 1); // cargo build should persist
    }

    #[test]
    fn persistence_ratio() {
        let mut model = DecayModel::new();
        model.record("a".into(), ts(0));
        model.record("b".into(), ts(30)); // ephemeral
        let barcode = PersistenceBarcode::from_model(&model);
        assert!(barcode.persistence_ratio() > 0.0);
        assert!(barcode.persistence_ratio() <= 1.0);
    }

    #[test]
    fn ascii_rendering() {
        let mut model = DecayModel::new();
        model.record("cargo build".into(), ts(0));
        let barcode = PersistenceBarcode::from_model(&model);
        let rendered = barcode.render_ascii(20);
        assert_eq!(rendered.len(), 20);
        assert!(rendered.contains('█'));
    }

    #[test]
    fn ascii_rendering_narrow() {
        let mut model = DecayModel::new();
        for i in 0..50 {
            model.record(format!("cmd_{}", i), ts(0));
        }
        let barcode = PersistenceBarcode::from_model(&model);
        let rendered = barcode.render_ascii(10);
        assert_eq!(rendered.len(), 10);
    }

    #[test]
    fn labeled_rendering() {
        let mut model = DecayModel::new();
        model.record("cargo build".into(), ts(0));
        model.record("cargo test".into(), ts(0));
        let barcode = PersistenceBarcode::from_model(&model);
        let labeled = barcode.render_labeled(40);
        assert!(labeled.contains("Persisting"));
    }

    #[test]
    fn persistence_diagram() {
        let mut model = DecayModel::new();
        model.record("cargo build".into(), ts(0));
        model.record("cargo build".into(), ts(5));
        let diag = PersistenceDiagram::from_model(&model);
        assert_eq!(diag.len(), 1);
        let (_, birth, death) = &diag.points[0];
        assert!(birth > death);
    }

    #[test]
    fn diagram_ascii_render() {
        let mut model = DecayModel::new();
        model.record("cargo build".into(), ts(0));
        model.record("cargo test".into(), ts(1));
        let diag = PersistenceDiagram::from_model(&model);
        let rendered = diag.render_ascii(10);
        assert!(rendered.contains("cargo"));
        assert!(rendered.contains('█'));
    }

    #[test]
    fn strength_char_mapping() {
        assert_eq!(strength_char(0.9, true), '█');
        assert_eq!(strength_char(0.7, true), '▓');
        assert_eq!(strength_char(0.4, true), '▒');
        assert_eq!(strength_char(0.15, true), '░');
        assert_eq!(strength_char(0.05, false), '·');
    }

    #[test]
    fn truncate_string() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello w…");
    }

    #[test]
    fn test_truncate_multibyte_utf8() {
        // These would have panicked with the old byte-index slicing.
        // "日本語テスト" = 18 bytes; max_len=9 → "日本…" (3+3+3 = 9 bytes)
        assert_eq!(truncate("日本語テスト", 9), "日本…");
        // "hello 世界" = 12 bytes; max_len=7 → take 6 bytes = "hello "
        assert_eq!(truncate("hello 世界", 7), "hello …");
        // Full string fits.
        assert_eq!(truncate("日本語テスト", 18), "日本語テスト");
        assert_eq!(truncate("日本語テスト", 20), "日本語テスト");
    }

    #[test]
    fn empty_diagram() {
        let model = DecayModel::new();
        let diag = PersistenceDiagram::from_model(&model);
        assert!(diag.is_empty());
        assert_eq!(diag.render_ascii(10), "(empty diagram)");
    }

    #[test]
    fn barcode_slot_persists() {
        let slot = BarcodeSlot {
            command: "test".into(),
            strength: 0.5,
            persists: true,
        };
        assert!(slot.persists);
        let weak = BarcodeSlot {
            command: "test".into(),
            strength: 0.02,
            persists: false,
        };
        assert!(!weak.persists);
    }
}
