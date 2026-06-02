//! # Math-Aware Command Analysis Layer
//!
//! Feature-gated module (`math-tools`) providing mathematical analysis
//! of terminal command patterns, error decomposition, verification
//! entropy tracking, and agent network spectral analysis.
//!
//! ## Submodules
//!
//! - [`command_markov`] — Ergodic Markov chain analysis of command frequency
//! - [`error_hodge`] — Hodge decomposition of error signals
//! - [`verification_entropy`] — Conservation-of-entropy edit/test ratio tracking
//! - [`spectral_dashboard`] — Agent collaboration network spectral metrics

#[cfg(feature = "math-tools")]
pub mod command_markov;
#[cfg(feature = "math-tools")]
pub mod error_hodge;
#[cfg(feature = "math-tools")]
pub mod spectral_dashboard;
#[cfg(feature = "math-tools")]
pub mod verification_entropy;

// Re-export the public API surface.
#[cfg(feature = "math-tools")]
pub use command_markov::{CommandMarkovChain, Anomaly};
#[cfg(feature = "math-tools")]
pub use error_hodge::{ErrorHodge, ErrorDecomposition};
#[cfg(feature = "math-tools")]
pub use verification_entropy::{VerificationEntropy, VerificationEvent};
#[cfg(feature = "math-tools")]
pub use spectral_dashboard::{SpectralDashboard, AgentGraph};
