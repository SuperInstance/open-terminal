//! Resource usage forecasting from historical command data.
//!
//! From historical observations:
//! - "When you run `cargo build`, memory usage increases by ~200MB"
//! - "This command typically needs 2GB RAM"
//!
//! Predicts resource needs *before* execution and warns if predicted
//! requirements exceed available capacity.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Observed resource usage for a single command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceObservation {
    /// Memory usage in bytes.
    pub memory_bytes: u64,
    /// CPU time in milliseconds.
    pub cpu_time_ms: u64,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
}

/// Aggregated resource statistics for a command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStats {
    /// Command name.
    pub command: String,
    /// Number of observations.
    pub observation_count: u64,
    /// Exponential moving average of memory usage (bytes).
    pub memory_ema: f64,
    /// Exponential moving average of CPU time (ms).
    pub cpu_ema: f64,
    /// Exponential moving average of duration (ms).
    pub duration_ema: f64,
    /// Memory variance estimate.
    pub memory_var: f64,
    /// Minimum observed memory.
    pub memory_min: u64,
    /// Maximum observed memory.
    pub memory_max: u64,
}

/// A resource prediction for an upcoming command.
#[derive(Debug, Clone)]
pub struct ResourcePrediction {
    /// The command being predicted.
    pub command: String,
    /// Predicted memory usage in bytes.
    pub predicted_memory_bytes: u64,
    /// Predicted CPU time in ms.
    pub predicted_cpu_ms: u64,
    /// Predicted wall-clock duration in ms.
    pub predicted_duration_ms: u64,
    /// Confidence: number of past observations.
    pub observation_count: u64,
    /// Whether the prediction is a warning (exceeds available).
    pub is_warning: bool,
    /// Human-readable warning message, if any.
    pub warning_message: Option<String>,
}

impl std::fmt::Display for ResourcePrediction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mem_gb = self.predicted_memory_bytes as f64 / 1e9;
        write!(f, "`{}` typically needs {:.1}GB RAM", self.command, mem_gb)?;
        if self.observation_count > 0 {
            write!(f, " (based on {} observations)", self.observation_count)?;
        }
        if let Some(ref msg) = self.warning_message {
            write!(f, "\n⚠️ {}", msg)?;
        }
        Ok(())
    }
}

/// Current system resource availability.
#[derive(Debug, Clone, Default)]
pub struct ResourceAvailability {
    /// Available memory in bytes.
    pub free_memory_bytes: u64,
    /// Total memory in bytes.
    pub total_memory_bytes: u64,
    /// Number of CPU cores.
    pub cpu_cores: usize,
}

/// The resource predictor tracks per-command resource usage patterns
/// and forecasts future resource needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePredictor {
    /// Per-command resource statistics.
    stats: HashMap<String, ResourceStats>,
    /// EMA smoothing factor.
    alpha: f64,
}

impl ResourcePredictor {
    /// Create a new predictor with default smoothing factor.
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
            alpha: 0.3,
        }
    }

    /// Create a predictor with a custom smoothing factor.
    pub fn with_alpha(alpha: f64) -> Self {
        assert!((0.0..=1.0).contains(&alpha), "alpha must be in [0, 1]");
        Self {
            stats: HashMap::new(),
            alpha,
        }
    }

    /// Number of commands with resource data.
    pub fn num_commands(&self) -> usize {
        self.stats.len()
    }

    /// Record a resource observation for a command.
    pub fn observe(&mut self, command: &str, obs: ResourceObservation) {
        let alpha = self.alpha;
        let entry = self.stats.entry(command.to_string()).or_insert_with(|| {
            ResourceStats {
                command: command.to_string(),
                observation_count: 0,
                memory_ema: obs.memory_bytes as f64,
                cpu_ema: obs.cpu_time_ms as f64,
                duration_ema: obs.duration_ms as f64,
                memory_var: 0.0,
                memory_min: obs.memory_bytes,
                memory_max: obs.memory_bytes,
            }
        });

        if entry.observation_count == 0 {
            entry.memory_ema = obs.memory_bytes as f64;
            entry.cpu_ema = obs.cpu_time_ms as f64;
            entry.duration_ema = obs.duration_ms as f64;
            entry.memory_var = 0.0;
        } else {
            let old_mem = entry.memory_ema;
            entry.memory_ema = alpha * obs.memory_bytes as f64 + (1.0 - alpha) * old_mem;
            entry.cpu_ema = alpha * obs.cpu_time_ms as f64 + (1.0 - alpha) * entry.cpu_ema;
            entry.duration_ema = alpha * obs.duration_ms as f64 + (1.0 - alpha) * entry.duration_ema;
            let diff = obs.memory_bytes as f64 - entry.memory_ema;
            entry.memory_var = alpha * diff * diff + (1.0 - alpha) * entry.memory_var;
        }

        entry.observation_count += 1;
        entry.memory_min = entry.memory_min.min(obs.memory_bytes);
        entry.memory_max = entry.memory_max.max(obs.memory_bytes);
    }

    /// Record a batch of observations.
    pub fn observe_batch(&mut self, command: &str, observations: &[ResourceObservation]) {
        for obs in observations {
            self.observe(command, obs.clone());
        }
    }

    /// Predict resource usage for a command.
    pub fn predict(
        &self,
        command: &str,
        available: Option<&ResourceAvailability>,
    ) -> Option<ResourcePrediction> {
        let stats = self.stats.get(command)?;
        let std_dev = stats.memory_var.sqrt();
        let predicted_memory = (stats.memory_ema + std_dev) as u64;

        let (is_warning, warning_message) = match available {
            Some(avail) if predicted_memory > avail.free_memory_bytes => {
                let pred_gb = predicted_memory as f64 / 1e9;
                let free_gb = avail.free_memory_bytes as f64 / 1e9;
                (
                    true,
                    Some(format!(
                        "This build typically uses {:.1}GB. You have {:.1}GB free.",
                        pred_gb, free_gb
                    )),
                )
            }
            _ => (false, None),
        };

        Some(ResourcePrediction {
            command: command.to_string(),
            predicted_memory_bytes: predicted_memory,
            predicted_cpu_ms: stats.cpu_ema as u64,
            predicted_duration_ms: stats.duration_ema as u64,
            observation_count: stats.observation_count,
            is_warning,
            warning_message,
        })
    }

    /// Get the raw statistics for a command.
    pub fn stats_for(&self, command: &str) -> Option<&ResourceStats> {
        self.stats.get(command)
    }

    /// Generate a delta description.
    pub fn memory_delta_description(&self, command: &str) -> Option<String> {
        let stats = self.stats.get(command)?;
        let delta_mb = stats.memory_ema / 1e6;
        Some(format!(
            "When you run `{}`, memory usage increases by ~{:.0}MB",
            command, delta_mb
        ))
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

impl Default for ResourcePredictor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_predictor() -> ResourcePredictor {
        let mut p = ResourcePredictor::new();
        p.observe("cargo build", ResourceObservation {
            memory_bytes: 2_000_000_000,
            cpu_time_ms: 30000,
            duration_ms: 45000,
        });
        p.observe("cargo build", ResourceObservation {
            memory_bytes: 2_100_000_000,
            cpu_time_ms: 28000,
            duration_ms: 42000,
        });
        p.observe("cargo build", ResourceObservation {
            memory_bytes: 1_900_000_000,
            cpu_time_ms: 32000,
            duration_ms: 48000,
        });
        p
    }

    #[test]
    fn new_predictor_is_empty() {
        let p = ResourcePredictor::new();
        assert_eq!(p.num_commands(), 0);
    }

    #[test]
    fn observe_creates_entry() {
        let mut p = ResourcePredictor::new();
        p.observe("ls", ResourceObservation {
            memory_bytes: 10_000_000,
            cpu_time_ms: 50,
            duration_ms: 100,
        });
        assert_eq!(p.num_commands(), 1);
        assert_eq!(p.stats_for("ls").unwrap().observation_count, 1);
    }

    #[test]
    fn ema_updates_with_multiple_observations() {
        let p = make_predictor();
        let stats = p.stats_for("cargo build").unwrap();
        assert_eq!(stats.observation_count, 3);
        assert!(stats.memory_ema >= 1_900_000_000.0);
        assert!(stats.memory_ema <= 2_100_000_000.0);
    }

    #[test]
    fn predict_returns_ema_plus_std() {
        let p = make_predictor();
        let pred = p.predict("cargo build", None).unwrap();
        assert_eq!(pred.observation_count, 3);
        assert!(pred.predicted_memory_bytes >= 1_900_000_000);
    }

    #[test]
    fn predict_unknown_returns_none() {
        let p = make_predictor();
        assert!(p.predict("nonexistent", None).is_none());
    }

    #[test]
    fn predict_warning_when_exceeds_available() {
        let p = make_predictor();
        let avail = ResourceAvailability {
            free_memory_bytes: 500_000_000,
            total_memory_bytes: 4_000_000_000,
            cpu_cores: 4,
        };
        let pred = p.predict("cargo build", Some(&avail)).unwrap();
        assert!(pred.is_warning);
        assert!(pred.warning_message.is_some());
    }

    #[test]
    fn predict_no_warning_when_enough_memory() {
        let p = make_predictor();
        let avail = ResourceAvailability {
            free_memory_bytes: 10_000_000_000,
            total_memory_bytes: 16_000_000_000,
            cpu_cores: 8,
        };
        let pred = p.predict("cargo build", Some(&avail)).unwrap();
        assert!(!pred.is_warning);
    }

    #[test]
    fn memory_delta_description() {
        let p = make_predictor();
        let desc = p.memory_delta_description("cargo build").unwrap();
        assert!(desc.contains("cargo build"));
        assert!(desc.contains("MB"));
    }

    #[test]
    fn memory_delta_unknown_command() {
        let p = make_predictor();
        assert!(p.memory_delta_description("nonexistent").is_none());
    }

    #[test]
    fn prediction_display_format() {
        let p = make_predictor();
        let pred = p.predict("cargo build", None).unwrap();
        let formatted = format!("{}", pred);
        assert!(formatted.contains("cargo build"));
        assert!(formatted.contains("GB"));
    }

    #[test]
    fn prediction_display_with_warning() {
        let p = make_predictor();
        let avail = ResourceAvailability {
            free_memory_bytes: 500_000_000,
            total_memory_bytes: 4_000_000_000,
            cpu_cores: 4,
        };
        let pred = p.predict("cargo build", Some(&avail)).unwrap();
        let formatted = format!("{}", pred);
        assert!(formatted.contains("⚠️"));
    }

    #[test]
    fn serialization_roundtrip() {
        let p = make_predictor();
        let json = p.to_json().unwrap();
        let restored = ResourcePredictor::from_json(&json).unwrap();
        assert_eq!(restored.num_commands(), p.num_commands());
        assert_eq!(restored.stats_for("cargo build").unwrap().observation_count, 3);
    }

    #[test]
    fn min_max_tracking() {
        let p = make_predictor();
        let stats = p.stats_for("cargo build").unwrap();
        assert_eq!(stats.memory_min, 1_900_000_000);
        assert_eq!(stats.memory_max, 2_100_000_000);
    }

    #[test]
    fn observe_batch() {
        let mut p = ResourcePredictor::new();
        p.observe_batch("npm test", &[
            ResourceObservation { memory_bytes: 500_000_000, cpu_time_ms: 10000, duration_ms: 15000 },
            ResourceObservation { memory_bytes: 600_000_000, cpu_time_ms: 12000, duration_ms: 18000 },
        ]);
        assert_eq!(p.stats_for("npm test").unwrap().observation_count, 2);
    }

    #[test]
    fn variance_increases_for_variable_usage() {
        let mut p = ResourcePredictor::new();
        for _ in 0..10 {
            p.observe("stable", ResourceObservation { memory_bytes: 1_000_000_000, cpu_time_ms: 1000, duration_ms: 2000 });
        }
        let stable_var = p.stats_for("stable").unwrap().memory_var;
        for i in 0..10u64 {
            p.observe("variable", ResourceObservation { memory_bytes: 1_000_000_000 + i * 500_000_000, cpu_time_ms: 1000, duration_ms: 2000 });
        }
        let variable_var = p.stats_for("variable").unwrap().memory_var;
        assert!(variable_var > stable_var);
    }

    #[test]
    fn custom_alpha() {
        let p = ResourcePredictor::with_alpha(0.1);
        assert_eq!(p.alpha, 0.1);
    }

    #[test]
    #[should_panic(expected = "alpha must be in [0, 1]")]
    fn invalid_alpha_panics() {
        ResourcePredictor::with_alpha(1.5);
    }
}
