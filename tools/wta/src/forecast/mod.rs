//! # Command Forecasting System
//!
//! "What will you type next?" The stationary distribution of your command
//! Markov chain tells us.
//!
//! This module provides ergodic-theory-based command forecasting for
//! Intelligent Terminal. The core insight:
//!
//! > "Your future commands are your past commands, averaged. The terminal
//! > doesn't guess — it computes your stationary distribution and reads
//! > the probabilities."
//!
//! ## Architecture
//!
//! - [`transition_matrix`] — Build the Markov chain from observed command
//!   sequences. States = unique commands, transitions = command→command
//!   frequency, Laplace-smoothed and row-normalized.
//!
//! - [`predictor`] — Predict the next command given the current one.
//!   Top-K predictions with confidence scores, ghost text for autocomplete.
//!
//! - [`anomaly`] — Detect workflow shifts via KL divergence and Wasserstein
//!   distance between current and stationary distributions.
//!
//! - [`resource_predictor`] — Forecast resource needs from historical data.
//!   Warn when predicted usage exceeds available capacity.
//!
//! ## Feature Gate
//!
//! All modules are gated behind `#[cfg(feature = "math-tools")]`.

pub mod transition_matrix;
pub mod predictor;
pub mod anomaly;
pub mod resource_predictor;

// Re-export the main types for convenience.
pub use transition_matrix::TransitionMatrix;
pub use predictor::{predict_next, predict_top3, predict_rich, Prediction, PredictionResult, RichPrediction};
pub use anomaly::{AnomalyDetector, WorkflowShift, ShiftSeverity, ShiftRecord};
pub use resource_predictor::{ResourcePredictor, ResourceObservation, ResourcePrediction, ResourceAvailability, ResourceStats};
