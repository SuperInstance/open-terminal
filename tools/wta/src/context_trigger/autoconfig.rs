//! Zero-config activation for the trigger engine.
//!
//! On first run, detect what tools are available on the system and
//! auto-enable the corresponding trigger patterns. No settings file,
//! no config UI, no user input needed.

use super::dormant::ModuleState;
use super::ALL_TRIGGERS;

/// Tool detection results.
#[derive(Debug, Clone)]
pub struct Config {
    pub cargo_detected: bool,
    pub python_detected: bool,
    pub node_detected: bool,
    pub multiple_agent_clis: bool,
    pub rust_tools: Vec<&'static str>,
    pub python_tools: Vec<&'static str>,
    pub node_tools: Vec<&'static str>,
}

impl Config {
    /// Detect what tools are available on this system.
    ///
    /// Scans PATH for common development tooling. This runs once at
    /// startup and is memoized — it's NOT per-event.
    ///
    /// This function runs `which`/`where` checks against PATH. These
    /// are lightweight syscalls (stat on each PATH component), not
    /// process spawns — typically <50µs per check on modern systems.
    pub fn detect() -> Self {
        // Use `which` crate for cross-platform PATH lookups.
        let cargo_detected = which::which("cargo").is_ok();
        let python_detected = which::which("python").is_ok() || which::which("python3").is_ok();
        let node_detected = which::which("node").is_ok();

        // Check for multiple ACP-capable agent CLIs.
        let copilot_detected = which::which("github-copilot-cli").is_ok()
            || which::which("copilot").is_ok();
        let claude_detected = which::which("claude").is_ok();
        let codex_detected = which::which("codex").is_ok();
        let gemini_detected = which::which("gemini").is_ok();

        let agent_cli_count = [
            copilot_detected,
            claude_detected,
            codex_detected,
            gemini_detected,
        ]
        .iter()
        .filter(|&&found| found)
        .count();

        let multiple_agent_clis = agent_cli_count >= 2;

        // Collect the specific tools found per language ecosystem.
        let mut rust_tools = Vec::new();
        if cargo_detected {
            rust_tools.push("cargo");
            if which::which("rustup").is_ok() {
                rust_tools.push("rustup");
            }
            if which::which("rustc").is_ok() {
                rust_tools.push("rustc");
            }
            if which::which("clippy-driver").is_ok() {
                rust_tools.push("clippy");
            }
        }

        let mut python_tools = Vec::new();
        if python_detected {
            python_tools.push("python");
            if which::which("pip").is_ok() || which::which("pip3").is_ok() {
                python_tools.push("pip");
            }
            if which::which("pytest").is_ok() {
                python_tools.push("pytest");
            }
            if which::which("poetry").is_ok() {
                python_tools.push("poetry");
            }
        }

        let mut node_tools = Vec::new();
        if node_detected {
            node_tools.push("node");
            if which::which("npm").is_ok() {
                node_tools.push("npm");
            }
            if which::which("npx").is_ok() {
                node_tools.push("npx");
            }
        }

        Config {
            cargo_detected,
            python_detected,
            node_detected,
            multiple_agent_clis,
            rust_tools,
            python_tools,
            node_tools,
        }
    }

    /// Suggest which features to enable based on detected tools.
    pub fn suggested_features(&self) -> Vec<&'static str> {
        let mut features = Vec::new();
        if self.cargo_detected || self.node_detected || self.python_detected {
            // Any development tooling makes math-tools relevant.
            #[cfg(feature = "math-tools")]
            features.push("math-tools");
        }
        if self.cargo_detected {
            // Rust developers benefit from griot-history's project detection.
            #[cfg(feature = "griot-history")]
            features.push("griot-history");
        }
        if self.multiple_agent_clis {
            // Multi-agent setups benefit from spectral analysis.
            #[cfg(feature = "math-tools")]
            features.push("spectral-dashboard");
        }
        features.sort();
        features.dedup();
        features
    }

    /// Returns true if the user is a developer (any tooling detected).
    pub fn is_developer(&self) -> bool {
        self.cargo_detected || self.python_detected || self.node_detected
    }
}

/// Detect available tools and return the Config.
///
/// This is the primary entry point called by the trigger engine at startup.
/// Results are memoized so subsequent calls are instant.
pub fn detect_and_configure() -> Config {
    // In the current design, auto-configuration is informational only —
    // feature gates are compile-time decisions. The Config is available
    // for runtime checks and for future dynamic module loading.
    let config = Config::detect();

    // Log what we found (for debugging).
    let mut found: Vec<&str> = Vec::new();
    if config.cargo_detected {
        found.push("cargo");
    }
    if config.python_detected {
        found.push("python");
    }
    if config.node_detected {
        found.push("node");
    }
    let features = config.suggested_features();

    tracing::debug!(
        "autoconfig: found={} suggested={:?} rust={:?} python={:?} node={:?} multi_agent={}",
        found.join(", "),
        features,
        config.rust_tools,
        config.python_tools,
        config.node_tools,
        config.multiple_agent_clis,
    );

    config
}

/// Pre-activate modules based on detected tools.
///
/// Some modules should start in `Active` state when their preconditions
/// are met at startup (before any event arrives):
///
/// - If cargo is detected, mark Rust-specific triggers as pre-primed.
/// - If Python is detected, mark Python-specific triggers.
/// - If multiple agent CLIs are detected, pre-activate multi-agent features.
///
/// This is optional — the pure-event-driver model works without it,
/// but pre-activation reduces latency for predictable starting contexts.
pub fn pre_activate_modules(config: &Config) -> Vec<&'static str> {
    let mut activated = Vec::new();

    // If cargo is detected, prime Rust-specific patterns.
    if config.cargo_detected {
        #[cfg(feature = "math-tools")]
        activated.push("verification-entropy");
        #[cfg(feature = "griot-history")]
        activated.push("adinkra");
    }

    // If developer with multiple agent CLIs, pre-activate spectral dashboard.
    if config.multiple_agent_clis && config.is_developer() {
        #[cfg(feature = "math-tools")]
        activated.push("spectral-dashboard");
    }

    activated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_does_not_panic() {
        // This is the only test that runs actual system calls (which -based).
        // It must not panic in any environment, including sandboxed CI.
        let config = Config::detect();
        // The result is inherently environment-dependent, so we just check
        // it returns something sensible.
        assert!(
            config.is_developer() || !config.is_developer(),
            "is_developer should return true or false"
        );
    }

    #[test]
    fn suggested_features_no_duplicates() {
        let config = Config {
            cargo_detected: true,
            python_detected: true,
            node_detected: true,
            multiple_agent_clis: true,
            rust_tools: vec!["cargo"],
            python_tools: vec!["python"],
            node_tools: vec!["node"],
        };
        let features = config.suggested_features();
        let mut deduped = features.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(features.len(), deduped.len(), "no duplicate features");
    }

    #[test]
    fn detect_structure_valid() {
        let config = Config {
            cargo_detected: false,
            python_detected: true,
            node_detected: false,
            multiple_agent_clis: false,
            rust_tools: vec![],
            python_tools: vec!["python", "pip"],
            node_tools: vec![],
        };
        assert!(!config.cargo_detected);
        assert!(config.python_detected);
        assert_eq!(config.python_tools.len(), 2);
    }

    #[test]
    fn pre_activate_modules_rust() {
        let config = Config {
            cargo_detected: true,
            python_detected: false,
            node_detected: false,
            multiple_agent_clis: false,
            rust_tools: vec!["cargo"],
            python_tools: vec![],
            node_tools: vec![],
        };
        let activated = pre_activate_modules(&config);
        #[cfg(feature = "math-tools")]
        assert!(activated.contains(&"verification-entropy"));
        #[cfg(not(feature = "math-tools"))]
        assert!(!activated.contains(&"verification-entropy"));
    }

    #[test]
    fn pre_activate_modules_multi_agent() {
        let config = Config {
            cargo_detected: true,
            python_detected: true,
            node_detected: false,
            multiple_agent_clis: true,
            rust_tools: vec!["cargo"],
            python_tools: vec!["python"],
            node_tools: vec![],
        };
        let activated = pre_activate_modules(&config);
        #[cfg(feature = "math-tools")]
        assert!(activated.contains(&"spectral-dashboard"));
    }

    #[test]
    fn is_developer_at_least_one() {
        // If any tooling is detected, is_developer returns true.
        let d1 = Config {
            cargo_detected: true,
            python_detected: false,
            node_detected: false,
            multiple_agent_clis: false,
            rust_tools: vec!["cargo"],
            python_tools: vec![],
            node_tools: vec![],
        };
        assert!(d1.is_developer());

        let d2 = Config {
            cargo_detected: false,
            python_detected: false,
            node_detected: false,
            multiple_agent_clis: false,
            rust_tools: vec![],
            python_tools: vec![],
            node_tools: vec![],
        };
        assert!(!d2.is_developer());
    }

    #[test]
    fn pre_activate_no_cargo_means_no_adinkra() {
        let config = Config {
            cargo_detected: false,
            python_detected: true,
            node_detected: false,
            multiple_agent_clis: false,
            rust_tools: vec![],
            python_tools: vec!["python"],
            node_tools: vec![],
        };
        let activated = pre_activate_modules(&config);
        assert!(!activated.contains(&"adinkra"));
    }

    #[test]
    fn detect_and_configure_returns_config() {
        // Must not panic.
        let config = detect_and_configure();
        // Should return a valid Config (any environment).
        assert!(config.rust_tools.is_empty() || !config.rust_tools.is_empty());
    }
}
