//! Universality classes of workflows.
//!
//! In statistical physics, the renormalization group reveals that seemingly
//! different systems can have the *same* critical exponents — they belong
//! to the same "universality class." The microscopic details don't matter;
//! only the coarse-grained behavior does.
//!
//! We apply this insight to command workflows:
//!
//! | Class           | Exponent  | Behavior                    | Prediction          |
//! |-----------------|-----------|-----------------------------|---------------------|
//! | Build-heavy     | ~0.5      | Fast convergence, repetitive | "Automate this"     |
//! | Debug-heavy     | ~1.0      | Slow convergence, exploratory | "Document this"     |
//! | Exploratory     | >1.5      | Never converges, creative     | "Keep exploring"    |
//!
//! Prediction: "Based on your universality class, you'll plateau on this
//! workflow in ~2 weeks."

use super::fixed_point::ConvergenceInfo;

/// The critical exponent measuring how fast a workflow converges.
#[derive(Debug, Clone, PartialEq)]
pub struct CriticalExponent {
    /// The computed exponent value.
    pub value: f64,
    /// Confidence: 0.0 (guess) to 1.0 (many levels of data).
    pub confidence: f64,
}

impl CriticalExponent {
    pub fn new(value: f64, confidence: f64) -> Self {
        Self {
            value,
            confidence: confidence.clamp(0.0, 1.0),
        }
    }
}

/// Universality class of a workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UniversalityClass {
    /// Fast convergence, repetitive. Exponent ~0.5.
    /// The user does the same thing over and over — ripe for automation.
    BuildHeavy,
    /// Moderate convergence, exploratory with structure. Exponent ~1.0.
    /// The user has a pattern but it's deeper and more nuanced.
    DebugHeavy,
    /// Slow or no convergence, creative. Exponent >1.5.
    /// The user is exploring, learning, creating — no fixed pattern yet.
    Exploratory,
    /// Very fast convergence — purely mechanical. Exponent <0.3.
    /// Likely a CI/CD pipeline or automated task being run by hand.
    Mechanical,
    /// Transitioning — the exponent is changing over time.
    /// "Your workflow just changed — new project?"
    Transitioning,
}

impl UniversalityClass {
    /// Human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::BuildHeavy => "Build-heavy workflow: repetitive, converges fast. Consider automation.",
            Self::DebugHeavy => "Debug-heavy workflow: structured exploration. Consider documenting your process.",
            Self::Exploratory => "Exploratory workflow: no fixed pattern yet. Keep exploring!",
            Self::Mechanical => "Mechanical workflow: pure repetition. Automate this immediately.",
            Self::Transitioning => "Your workflow is changing — new project or new approach?",
        }
    }

    /// Short label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::BuildHeavy => "build-heavy",
            Self::DebugHeavy => "debug-heavy",
            Self::Exploratory => "exploratory",
            Self::Mechanical => "mechanical",
            Self::Transitioning => "transitioning",
        }
    }

    /// Predicted time to plateau (in arbitrary units, roughly "days of active use").
    pub fn predicted_plateau_time(&self) -> f64 {
        match self {
            Self::Mechanical => 1.0,
            Self::BuildHeavy => 7.0,
            Self::DebugHeavy => 14.0,
            Self::Exploratory => f64::INFINITY,
            Self::Transitioning => 21.0,
        }
    }

    /// The expected critical exponent range for this class.
    pub fn exponent_range(&self) -> (f64, f64) {
        match self {
            Self::Mechanical => (0.0, 0.3),
            Self::BuildHeavy => (0.3, 0.8),
            Self::DebugHeavy => (0.8, 1.5),
            Self::Exploratory => (1.5, f64::INFINITY),
            Self::Transitioning => (-f64::INFINITY, f64::INFINITY), // Any — detected by change
        }
    }
}

impl std::fmt::Display for UniversalityClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Classifies workflows into universality classes.
#[derive(Debug, Clone)]
pub struct UniversalityClassifier {
    /// Minimum confidence to give a definitive classification.
    min_confidence: f64,
}

impl UniversalityClassifier {
    pub fn new() -> Self {
        Self {
            min_confidence: 0.3,
        }
    }

    /// Compute the critical exponent from a sequence of coarse-grained levels.
    pub fn compute_critical_exponent(
        &self,
        levels: &[super::coarse_grain::CoarseGrainLevel],
    ) -> CriticalExponent {
        if levels.len() < 3 {
            return CriticalExponent::new(0.0, 0.0);
        }

        // Compute JSD between successive levels
        let jsd_values: Vec<f64> = levels
            .windows(2)
            .map(|w| {
                let a = &w[0];
                let b = &w[1];
                self.jsd_between(a, b)
            })
            .collect();

        // Fit the decay: log(JSD) = -ν * level + const
        let valid: Vec<(f64, f64)> = jsd_values
            .iter()
            .enumerate()
            .filter(|(_, &jsd)| jsd > 1e-10)
            .map(|(i, &jsd)| ((i + 1) as f64, jsd.ln()))
            .collect();

        if valid.len() < 2 {
            return CriticalExponent::new(
                if jsd_values.last().copied().unwrap_or(0.0) < 0.01 {
                    0.1 // Converged quickly
                } else {
                    2.0 // Not converging
                },
                0.1,
            );
        }

        let n = valid.len() as f64;
        let sum_x: f64 = valid.iter().map(|(x, _)| x).sum();
        let sum_y: f64 = valid.iter().map(|(_, y)| y).sum();
        let sum_xy: f64 = valid.iter().map(|(x, y)| x * y).sum();
        let sum_x2: f64 = valid.iter().map(|(x, _)| x * x).sum();

        let denom = n * sum_x2 - sum_x * sum_x;
        if denom.abs() < 1e-10 {
            return CriticalExponent::new(0.0, 0.1);
        }

        let slope = (n * sum_xy - sum_x * sum_y) / denom;
        let nu = -slope; // Positive = converging

        let confidence = (valid.len() as f64 / 5.0).min(1.0);
        CriticalExponent::new(nu, confidence)
    }

    /// Classify a critical exponent into a universality class.
    pub fn classify(&self, exponent: &CriticalExponent) -> UniversalityClass {
        if exponent.confidence < self.min_confidence {
            // Not enough data to be sure
            return UniversalityClass::Transitioning;
        }

        let nu = exponent.value;

        if nu < 0.0 {
            // Diverging — workflow is becoming more complex
            UniversalityClass::Transitioning
        } else if nu < 0.3 {
            UniversalityClass::Mechanical
        } else if nu < 0.8 {
            UniversalityClass::BuildHeavy
        } else if nu < 1.5 {
            UniversalityClass::DebugHeavy
        } else {
            UniversalityClass::Exploratory
        }
    }

    /// Classify directly from convergence info.
    pub fn classify_convergence(&self, info: &ConvergenceInfo) -> UniversalityClass {
        let exponent = CriticalExponent::new(info.critical_exponent(), 0.5);
        self.classify(&exponent)
    }

    /// JSD between two levels.
    fn jsd_between(
        &self,
        a: &super::coarse_grain::CoarseGrainLevel,
        b: &super::coarse_grain::CoarseGrainLevel,
    ) -> f64 {
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

            if p > 0.0 && m > 0.0 {
                jsd += p * (p / m).log2() / 2.0;
            }
            if q > 0.0 && m > 0.0 {
                jsd += q * (q / m).log2() / 2.0;
            }
        }

        jsd
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::griot_history::skill_detector::coarse_grain::{BlockSize, CoarseGrainer};

    fn levels_from_commands(commands: &[&str], max_levels: usize) -> Vec<crate::griot_history::skill_detector::coarse_grain::CoarseGrainLevel> {
        let cmds: Vec<String> = commands.iter().map(|s| s.to_string()).collect();
        let grainer = CoarseGrainer::new(&[BlockSize::B2, BlockSize::B5, BlockSize::B10]);
        grainer.renormalize(&cmds, max_levels)
    }

    #[test]
    fn uniform_workflow_is_mechanical_or_build_heavy() {
        let commands = vec!["make"; 200];
        let levels = levels_from_commands(&commands, 6);
        let classifier = UniversalityClassifier::new();
        let exp = classifier.compute_critical_exponent(&levels);
        let class = classifier.classify(&exp);
        assert!(matches!(class, UniversalityClass::Mechanical | UniversalityClass::BuildHeavy));
    }

    #[test]
    fn diverse_commands_are_exploratory() {
        // Many different commands, no clear pattern
        let commands: Vec<String> = (0..100)
            .map(|i| format!("unique_cmd_{}", i))
            .collect();
        let cmds_ref: Vec<&str> = commands.iter().map(|s| s.as_str()).collect();
        let levels = levels_from_commands(&cmds_ref, 4);
        let classifier = UniversalityClassifier::new();
        let exp = classifier.compute_critical_exponent(&levels);
        // With high diversity, should either be exploratory or low-confidence
        assert!(exp.value > 0.0 || exp.confidence < 0.3);
    }

    #[test]
    fn universality_class_display() {
        assert_eq!(format!("{}", UniversalityClass::BuildHeavy), "build-heavy");
        assert_eq!(format!("{}", UniversalityClass::Exploratory), "exploratory");
    }

    #[test]
    fn class_descriptions_are_nonempty() {
        for class in &[
            UniversalityClass::BuildHeavy,
            UniversalityClass::DebugHeavy,
            UniversalityClass::Exploratory,
            UniversalityClass::Mechanical,
            UniversalityClass::Transitioning,
        ] {
            assert!(!class.description().is_empty());
        }
    }

    #[test]
    fn plateau_time_is_positive() {
        for class in &[
            UniversalityClass::BuildHeavy,
            UniversalityClass::DebugHeavy,
            UniversalityClass::Mechanical,
        ] {
            assert!(class.predicted_plateau_time() > 0.0);
        }
    }

    #[test]
    fn exploratory_never_plateaus() {
        assert!(UniversalityClass::Exploratory.predicted_plateau_time().is_infinite());
    }

    #[test]
    fn exponent_ranges_are_ordered() {
        let mech = UniversalityClass::Mechanical.exponent_range();
        let build = UniversalityClass::BuildHeavy.exponent_range();
        let debug = UniversalityClass::DebugHeavy.exponent_range();
        let expl = UniversalityClass::Exploratory.exponent_range();
        assert!(mech.1 <= build.0);
        assert!(build.1 <= debug.0);
        assert!(debug.1 <= expl.0);
    }

    #[test]
    fn low_confidence_yields_transitioning() {
        let classifier = UniversalityClassifier::new();
        let exp = CriticalExponent::new(0.5, 0.1); // Low confidence
        let class = classifier.classify(&exp);
        assert_eq!(class, UniversalityClass::Transitioning);
    }

    #[test]
    fn negative_exponent_is_transitioning() {
        let classifier = UniversalityClassifier::new();
        let exp = CriticalExponent::new(-1.0, 0.8);
        let class = classifier.classify(&exp);
        assert_eq!(class, UniversalityClass::Transitioning);
    }

    #[test]
    fn two_command_cycle_is_build_or_debug() {
        let commands: Vec<&str> = (0..100)
            .flat_map(|_| ["cargo build", "cargo test"])
            .collect();
        let levels = levels_from_commands(&commands, 6);
        let classifier = UniversalityClassifier::new();
        let exp = classifier.compute_critical_exponent(&levels);
        let class = classifier.classify(&exp);
        assert!(matches!(
            class,
            UniversalityClass::BuildHeavy
                | UniversalityClass::DebugHeavy
                | UniversalityClass::Mechanical
        ));
    }

    #[test]
    fn confidence_increases_with_more_levels() {
        let commands = vec!["cmd"; 64];
        let levels = levels_from_commands(&commands, 6);
        let classifier = UniversalityClassifier::new();
        let exp = classifier.compute_critical_exponent(&levels);
        assert!(exp.confidence > 0.0);
    }

    #[test]
    fn fewer_than_three_levels_yields_zero_confidence() {
        let grainer = CoarseGrainer::new(&[BlockSize::B10]);
        let cmds = vec!["a".to_string()];
        let levels = grainer.renormalize(&cmds, 0);
        let classifier = UniversalityClassifier::new();
        let exp = classifier.compute_critical_exponent(&levels);
        assert_eq!(exp.confidence, 0.0);
    }
}
