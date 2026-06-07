//! Conservation monitor for open-terminal.
//!
//! Monitors system resource usage as a conservation law: CPU_active + idle = total.
//! Tracks γ (active work) and H (idle/waste). Alerts when γ + H > C (overcommit).
//!
//! The conservation model treats every computational resource as a physical quantity
//! governed by a conservation law. Just as energy cannot be created from nothing,
//! system resources cannot be overcommitted without consequences.

/// A snapshot of the conservation state at a given moment.
#[derive(Debug, Clone)]
pub struct ConservationReport {
    /// γ — active work rate (0.0–1.0 normalized CPU usage).
    pub gamma: f64,
    /// H — idle/waste rate (0.0–1.0 normalized).
    pub eta: f64,
    /// C — total budget capacity.
    pub capacity: f64,
    /// Memory usage as fraction of total (0.0–1.0).
    pub memory_fraction: f64,
    /// Number of samples collected so far.
    pub sample_count: u64,
    /// Timestamp of this report (monotonic counter).
    pub tick: u64,
    /// Whether the system is currently overcommitted.
    pub overcommitted: bool,
    /// The violation magnitude (0.0 if conserved).
    pub violation_magnitude: f64,
}

/// The conservation monitor tracks resource usage over time.
#[derive(Debug, Clone)]
pub struct ConservationMonitor {
    /// Total budget capacity (C).
    pub total_budget: f64,
    /// History of γ samples (ring buffer, last N).
    gamma_history: Vec<f64>,
    /// History of H samples.
    eta_history: Vec<f64>,
    /// Maximum history length.
    max_history: usize,
    /// Current sample count.
    sample_count: u64,
    /// Current tick.
    tick: u64,
    /// Tolerance for conservation check.
    tolerance: f64,
}

impl ConservationMonitor {
    /// Create a new conservation monitor with the given total budget.
    ///
    /// The budget represents the total system capacity (C in the conservation law).
    /// Typical values are 1.0 (normalized) or the number of CPU cores.
    pub fn new(total_budget: f64) -> Self {
        Self {
            total_budget,
            gamma_history: Vec::new(),
            eta_history: Vec::new(),
            max_history: 100,
            sample_count: 0,
            tick: 0,
            tolerance: 0.05,
        }
    }

    /// Set the tolerance for conservation violation detection.
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Set the maximum history length for the ring buffer.
    pub fn with_history_size(mut self, size: usize) -> Self {
        self.max_history = size.max(1);
        self
    }

    /// Take a sample of current resource usage and return a conservation report.
    ///
    /// - `cpu`: Current CPU usage as a fraction (0.0–1.0).
    /// - `mem`: Current memory usage as a fraction (0.0–1.0).
    ///
    /// The conservation law: γ (CPU) + H (idle) = C (total budget).
    /// Idle is computed as: H = C - γ (ideal case) or measured independently.
    /// Overcommit occurs when observed γ > C.
    pub fn sample(&mut self, cpu: f64, mem: f64) -> ConservationReport {
        self.tick += 1;
        self.sample_count += 1;

        let gamma = cpu.clamp(0.0, self.total_budget);
        let eta = (self.total_budget - gamma).max(0.0);
        let memory_fraction = mem.clamp(0.0, 1.0);

        // Push to history (ring buffer)
        if self.gamma_history.len() >= self.max_history {
            self.gamma_history.remove(0);
            self.eta_history.remove(0);
        }
        self.gamma_history.push(gamma);
        self.eta_history.push(eta);

        // Use weighted average: 70% current + 30% historical average
        let avg_gamma = self.weighted_average(&self.gamma_history, gamma);
        let avg_eta = self.weighted_average(&self.eta_history, eta);

        let effective_gamma = avg_gamma;
        let effective_eta = avg_eta;
        let violation = (effective_gamma + effective_eta - self.total_budget).max(0.0);
        let overcommitted = violation > self.tolerance;

        ConservationReport {
            gamma: effective_gamma,
            eta: effective_eta,
            capacity: self.total_budget,
            memory_fraction,
            sample_count: self.sample_count,
            tick: self.tick,
            overcommitted,
            violation_magnitude: violation,
        }
    }

    /// Check if the system is healthy (not overcommitted).
    pub fn is_healthy(&self) -> bool {
        if self.gamma_history.is_empty() {
            return true;
        }
        let last_gamma = *self.gamma_history.last().unwrap();
        let last_eta = *self.eta_history.last().unwrap();
        (last_gamma + last_eta) <= self.total_budget + self.tolerance
    }

    /// Get the average γ over the history window.
    pub fn average_gamma(&self) -> f64 {
        if self.gamma_history.is_empty() {
            return 0.0;
        }
        self.gamma_history.iter().sum::<f64>() / self.gamma_history.len() as f64
    }

    /// Get the average η over the history window.
    pub fn average_eta(&self) -> f64 {
        if self.eta_history.is_empty() {
            return 0.0;
        }
        self.eta_history.iter().sum::<f64>() / self.eta_history.len() as f64
    }

    /// Get the current trend (positive = γ increasing = system loading up).
    pub fn gamma_trend(&self) -> f64 {
        if self.gamma_history.len() < 2 {
            return 0.0;
        }
        let n = self.gamma_history.len();
        let recent: f64 = self.gamma_history[n - 5.min(n)..].iter().sum::<f64>()
            / 5.min(n) as f64;
        let older: f64 = self.gamma_history[..n.saturating_sub(5).max(1)]
            .iter()
            .sum::<f64>()
            / n.saturating_sub(5).max(1) as f64;
        recent - older
    }

    /// Get the number of samples collected.
    pub fn sample_count(&self) -> u64 {
        self.sample_count
    }

    /// Reset the monitor, clearing all history.
    pub fn reset(&mut self) {
        self.gamma_history.clear();
        self.eta_history.clear();
        self.sample_count = 0;
        self.tick = 0;
    }

    fn weighted_average(&self, history: &[f64], current: f64) -> f64 {
        if history.len() <= 1 {
            return current;
        }
        let historical_avg = history.iter().sum::<f64>() / history.len() as f64;
        0.7 * current + 0.3 * historical_avg
    }
}

impl ConservationReport {
    /// Returns a human-readable summary of the conservation state.
    pub fn summary(&self) -> String {
        if self.overcommitted {
            format!(
                "OVERCOMMITTED: γ={:.2} + H={:.2} = {:.2} > C={:.2} (violation: {:.2})",
                self.gamma, self.eta, self.gamma + self.eta, self.capacity, self.violation_magnitude
            )
        } else {
            format!(
                "CONSERVED: γ={:.2} + H={:.2} = {:.2} ≤ C={:.2}",
                self.gamma, self.eta, self.gamma + self.eta, self.capacity
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_monitor() {
        let monitor = ConservationMonitor::new(1.0);
        assert_eq!(monitor.total_budget, 1.0);
        assert!(monitor.is_healthy());
        assert_eq!(monitor.sample_count(), 0);
    }

    #[test]
    fn test_sample_within_budget() {
        let mut monitor = ConservationMonitor::new(1.0);
        let report = monitor.sample(0.4, 0.5);
        assert!(!report.overcommitted);
        assert_eq!(report.violation_magnitude, 0.0);
        assert!(report.gamma > 0.0);
        assert!(report.eta > 0.0);
    }

    #[test]
    fn test_sample_overcommit() {
        // Use budget of 0.5, so cpu=0.4 should be fine but cpu=0.6 should overcommit
        let mut monitor = ConservationMonitor::new(0.5).with_tolerance(0.01);
        let report = monitor.sample(0.6, 0.7);
        // gamma=0.6 > capacity=0.5 → overcommitted
        assert!(report.overcommitted);
        assert!(report.violation_magnitude > 0.0);
    }

    #[test]
    fn test_is_healthy_after_samples() {
        let mut monitor = ConservationMonitor::new(1.0);
        monitor.sample(0.3, 0.4);
        monitor.sample(0.5, 0.6);
        assert!(monitor.is_healthy());
    }

    #[test]
    fn test_is_healthy_unhealthy() {
        let mut monitor = ConservationMonitor::new(0.5).with_tolerance(0.01);
        monitor.sample(0.7, 0.5);
        assert!(!monitor.is_healthy());
    }

    #[test]
    fn test_averages() {
        let mut monitor = ConservationMonitor::new(1.0);
        monitor.sample(0.5, 0.5);
        monitor.sample(0.7, 0.3);
        assert!(monitor.average_gamma() > 0.0);
        assert!(monitor.average_eta() > 0.0);
    }

    #[test]
    fn test_ring_buffer_history() {
        let mut monitor = ConservationMonitor::new(1.0).with_history_size(3);
        monitor.sample(0.1, 0.1);
        monitor.sample(0.2, 0.2);
        monitor.sample(0.3, 0.3);
        monitor.sample(0.4, 0.4); // Should evict oldest
        assert_eq!(monitor.gamma_history.len(), 3);
    }

    #[test]
    fn test_report_summary_conserved() {
        let mut monitor = ConservationMonitor::new(1.0);
        let report = monitor.sample(0.3, 0.5);
        let summary = report.summary();
        assert!(summary.contains("CONSERVED"));
    }

    #[test]
    fn test_report_summary_overcommitted() {
        let mut monitor = ConservationMonitor::new(0.5).with_tolerance(0.01);
        let report = monitor.sample(0.7, 0.3);
        let summary = report.summary();
        assert!(summary.contains("OVERCOMMITTED"));
    }

    #[test]
    fn test_gamma_trend() {
        let mut monitor = ConservationMonitor::new(1.0);
        // Ramp up CPU
        for i in 0..10 {
            monitor.sample(0.1 * (i as f64 + 1.0).min(1.0), 0.3);
        }
        let trend = monitor.gamma_trend();
        // Trend should be positive (loading up)
        assert!(trend > 0.0);
    }

    #[test]
    fn test_reset() {
        let mut monitor = ConservationMonitor::new(1.0);
        monitor.sample(0.5, 0.5);
        monitor.sample(0.6, 0.4);
        assert_eq!(monitor.sample_count(), 2);
        monitor.reset();
        assert_eq!(monitor.sample_count(), 0);
        assert!(monitor.gamma_history.is_empty());
    }

    #[test]
    fn test_multiple_budget_values() {
        let mut monitor = ConservationMonitor::new(4.0); // 4-core system
        let report = monitor.sample(2.0, 0.5);
        assert!(!report.overcommitted);
        assert_eq!(report.capacity, 4.0);
    }

    #[test]
    fn test_clamped_values() {
        let mut monitor = ConservationMonitor::new(1.0);
        let report = monitor.sample(1.5, -0.1); // Out of range
        assert!(report.gamma <= 1.0);
        assert!(report.memory_fraction >= 0.0);
    }
}
