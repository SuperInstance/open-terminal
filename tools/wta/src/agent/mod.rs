//! Agent module for open-terminal (wta).
//!
//! Provides conservation monitoring and command suggestions as part of the
//! agents-as-applications paradigm. The terminal doesn't just run commands —
//! it *is* the agent interface, monitoring system resources as a conservation
//! law and suggesting intelligent actions.

pub mod conservation_monitor;
pub mod command_suggest;
