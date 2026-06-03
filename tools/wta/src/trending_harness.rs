//! # Trending Repo Harness
//!
//! Turns the Intelligent Terminal into a universal repo decomposer.
//! Clone any GitHub repo, analyze its structure, decompose into modules,
//! and absorb them as new terminal features.
//!
//! ## Philosophy
//!
//! Every GitHub repo is a potential terminal module. The trending harness
//! finds repos with high potential, clones and analyzes their structure,
//! decomposes them into independent modules, and generates integration
//! plans — so the terminal can grow by absorbing the best of the open
//! source ecosystem.
//!
//! ## Usage
//!
//! ```ignore
//! use wta::trending_harness::{fetch_trending, analyze_repo, decompose_modules};
//!
//! let repos = fetch_trending("terminal", 5).unwrap();
//! for repo in &repos {
//!     let analysis = analyze_repo(&repo.url).unwrap();
//!     let modules = decompose_modules(&repo.url).unwrap();
//!     println!("{} — {} modules available", repo.url, modules.len());
//! }
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A trending repository discovered via the GitHub Search API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendingRepo {
    /// Full clone URL (https://github.com/owner/repo.git).
    pub url: String,
    /// Star count at fetch time.
    pub stars: u64,
    /// Repository topics / tags.
    pub topics: Vec<String>,
    /// When this record was created.
    pub timestamp: DateTime<Utc>,
    /// Human-readable name (owner/repo).
    pub full_name: String,
    /// Short description.
    pub description: String,
}

/// Structural analysis of a repo's source tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoAnalysis {
    /// URL that was analyzed.
    pub url: String,
    /// Language as detected from Cargo.toml / build files.
    pub language: String,
    /// Number of top-level source modules or source files.
    pub module_count: usize,
    /// Number of test files or test functions found.
    pub test_count: usize,
    /// CI configuration files discovered (.github/workflows, Jenkinsfile, etc.).
    pub ci_configs: Vec<String>,
    /// Key patterns (e.g., "wasm", "async", "cli", "tui").
    pub patterns: Vec<String>,
    /// Raw key-value pairs parsed from the build metadata.
    pub metadata: HashMap<String, String>,
    /// Whether the repo has a build file we understand.
    pub has_build_system: bool,
}

/// A proposed module extracted from a repo's source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleProposal {
    /// Suggested name for the module.
    pub name: String,
    /// One-line description of what this module does.
    pub description: String,
    /// Relative paths of the source files that belong to this module.
    pub files: Vec<String>,
    /// Estimated complexity (lines of code).
    pub loc_estimate: usize,
    /// Dependencies this module would need.
    pub dependencies: Vec<String>,
}

/// An integration plan describing how to add external modules as terminal features.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationPlan {
    /// Source repo URL.
    pub source_url: String,
    /// Proposed new Cargo features to add.
    pub new_features: Vec<FeatureEntry>,
    /// Steps to carry out the integration.
    pub steps: Vec<IntegrationStep>,
    /// Estimated effort (minutes).
    pub estimated_effort_minutes: u32,
}

/// A single Cargo feature entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureEntry {
    /// Feature name (e.g., "trending-math").
    pub name: String,
    /// Dependencies this feature should activate.
    pub dependencies: Vec<String>,
    /// Modules this feature gates.
    pub modules: Vec<String>,
    /// Whether this is optional.
    pub optional: bool,
}

/// A single step in an integration plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationStep {
    /// Step number for ordering.
    pub step: u32,
    /// Action to take.
    pub action: String,
    /// Detailed instructions.
    pub details: String,
}

// ---------------------------------------------------------------------------
// Fetch trending repos via GitHub Search API
// ---------------------------------------------------------------------------

/// Fetch trending repositories matching a topic, sorted by stars.
///
/// Uses the public GitHub Search API (no authentication required for basic
/// usage, but rate-limited to 10 req/min without a token).
pub fn fetch_trending(topic: &str, limit: usize) -> Result<Vec<TrendingRepo>, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(fetch_trending_async(topic, limit))
}

async fn fetch_trending_async(topic: &str, limit: usize) -> Result<Vec<TrendingRepo>, String> {
    let url = format!(
        "https://api.github.com/search/repositories?q=topic:{}&sort=stars&order=desc&per_page={}",
        urlencoding(topic), limit.min(100)
    );

    let client = reqwest::Client::builder()
        .user_agent("intelligent-terminal-trending-harness/0.1")
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API returned {}: {}", status, body));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {}", e))?;

    let items = body["items"]
        .as_array()
        .ok_or_else(|| "No 'items' array in GitHub response".to_string())?;

    let repos: Vec<TrendingRepo> = items
        .iter()
        .take(limit)
        .map(|item| TrendingRepo {
            url: item["clone_url"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            stars: item["stargazers_count"].as_u64().unwrap_or(0),
            topics: item["topics"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            timestamp: Utc::now(),
            full_name: item["full_name"]
                .as_str()
                .unwrap_or("unknown/repo")
                .to_string(),
            description: item["description"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        })
        .collect();

    Ok(repos)
}

// ---------------------------------------------------------------------------
// Analyze a repository
// ---------------------------------------------------------------------------

/// Clone and analyze a repository, returning its structural breakdown.
///
/// The repo is cloned to a temporary directory, then the filesystem is scanned
/// for build configuration, module structure, tests, and CI files.
pub fn analyze_repo(url: &str) -> Result<RepoAnalysis, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(analyze_repo_async(url))
}

async fn analyze_repo_async(url: &str) -> Result<RepoAnalysis, String> {
    let tmp_dir = tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let repo_path = tmp_dir.path().join("repo");

    // Clone
    let clone_status = Command::new("git")
        .args(["clone", "--depth", "1", url, repo_path.to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map_err(|e| format!("Failed to run git clone: {}", e))?;

    if !clone_status.success() {
        return Err(format!("git clone failed for {}", url));
    }

    analyze_local(&repo_path, url)
}

/// Analyze a local repository directory (useful for already-cloned repos).
pub fn analyze_local(repo_path: &Path, url: &str) -> Result<RepoAnalysis, String> {
    let mut language = "unknown".to_string();
    let mut module_count = 0usize;
    let mut test_count = 0usize;
    let mut ci_configs = Vec::new();
    let mut patterns = Vec::new();
    let mut metadata = HashMap::new();
    let mut has_build_system = false;

    // --- Cargo.toml detection ---
    if repo_path.join("Cargo.toml").exists() {
        language = "Rust".to_string();
        has_build_system = true;
        let content = std::fs::read_to_string(repo_path.join("Cargo.toml")).unwrap_or_default();
        if content.contains("[lib]") {
            patterns.push("library".to_string());
        }
        if content.contains("[[bin]]") {
            patterns.push("binary".to_string());
        }
        if content.contains("wasm") || content.contains("wasm-bindgen") {
            patterns.push("wasm".to_string());
        }
        if content.contains("tokio") {
            patterns.push("async".to_string());
        }
        if content.contains("clap") || content.contains("structopt") {
            patterns.push("cli".to_string());
        }
        if content.contains("ratatui") || content.contains("crossterm") || content.contains("tui") {
            patterns.push("tui".to_string());
        }
        metadata.insert("edition".to_string(), extract_toml_value(&content, "edition"));
        metadata.insert("version".to_string(), extract_toml_value(&content, "version"));
    }

    // --- Module / file counting ---
    if let Ok(src_dir) = repo_path.join("src").read_dir() {
        for entry in src_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Count subdirectories as modules
                module_count += path.read_dir().map(|d| d.count()).unwrap_or(0);
            } else if let Some(ext) = path.extension() {
                let name = path.file_stem().unwrap_or_default().to_string_lossy();
                match ext.to_str().unwrap_or("") {
                    "rs" | "py" | "js" | "ts" | "go" | "java" | "kt" => {
                        module_count += 1;
                        if name.ends_with("_test") || name.ends_with("_spec") || name == "test" || name == "tests" {
                            test_count += 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Count actual test functions in Rust files
        if language == "Rust" {
            let src_entries: Vec<_> = repo_path.join("src").read_dir().ok().into_iter().flatten().flatten().collect();
            // Also count tests in lib.rs/main.rs
            for main_file in &["lib.rs", "main.rs"] {
                let path = repo_path.join("src").join(main_file);
                if path.exists() {
                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    test_count += content.matches("#[test]").count();
                    test_count += content.matches("#[tokio::test]").count();
                    if content.contains("#[cfg(test)]") {
                        // Add inline test module count
                        test_count += content.matches("fn ").count();
                    }
                }
            }
        }
    }

    // --- CI detection ---
    let github_ci = repo_path.join(".github/workflows");
    if github_ci.exists() {
        if let Ok(entries) = github_ci.read_dir() {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".yml") || name.ends_with(".yaml") {
                        ci_configs.push(format!(".github/workflows/{}", name));
                    }
                }
            }
        }
    }
    for ci_file in &[".travis.yml", "Jenkinsfile", ".circleci/config.yml", ".gitlab-ci.yml", "Makefile"] {
        if repo_path.join(ci_file).exists() {
            ci_configs.push(ci_file.to_string());
        }
    }

    // --- Additional pattern detection ---
    if repo_path.join("README.md").exists() {
        let readme = std::fs::read_to_string(repo_path.join("README.md")).unwrap_or_default();
        if readme.to_lowercase().contains("machine learning")
            || readme.to_lowercase().contains("ml")
            || readme.to_lowercase().contains("neural")
        {
            patterns.push("machine-learning".to_string());
        }
        if readme.to_lowercase().contains("api") {
            patterns.push("api".to_string());
        }
        if readme.to_lowercase().contains("graph") {
            patterns.push("graph".to_string());
        }
    }

    Ok(RepoAnalysis {
        url: url.to_string(),
        language,
        module_count,
        test_count,
        ci_configs,
        patterns,
        metadata,
        has_build_system,
    })
}

// ---------------------------------------------------------------------------
// Decompose a repo into modules
// ---------------------------------------------------------------------------

/// Decompose a cloned repository into independent module proposals.
///
/// Each module corresponds to a logical unit in the source tree (a directory
/// under `src/`, or a top-level file that isn't `main.rs`/`lib.rs`).
pub fn decompose_modules(url: &str) -> Result<Vec<ModuleProposal>, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(decompose_modules_async(url))
}

async fn decompose_modules_async(url: &str) -> Result<Vec<ModuleProposal>, String> {
    let tmp_dir = tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let repo_path = tmp_dir.path().join("repo");

    let clone_status = Command::new("git")
        .args(["clone", "--depth", "1", url, repo_path.to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map_err(|e| format!("Failed to run git clone: {}", e))?;

    if !clone_status.success() {
        return Err(format!("git clone failed for {}", url));
    }

    decompose_local(&repo_path, url)
}

/// Decompose a local repo (already cloned) into module proposals.
pub fn decompose_local(repo_path: &Path, url: &str) -> Result<Vec<ModuleProposal>, String> {
    let mut modules = Vec::new();
    let src_dir = repo_path.join("src");

    if !src_dir.exists() {
        return Ok(modules);
    }

    let entries: Vec<_> = src_dir.read_dir().map_err(|e| format!("Cannot read src/: {}", e))?
        .filter_map(|e| e.ok())
        .collect();

    for entry in &entries {
        let path = entry.path();
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy().to_string();

        if path.is_dir() {
            // Directory = submodule
            let mut files = Vec::new();
            let mut loc = 0usize;
            let mut deps = Vec::new();

            if let Ok(sub_entries) = path.read_dir() {
                for sub in sub_entries.flatten() {
                    let sub_path = sub.path();
                    if let Some(ext) = sub_path.extension() {
                        if ext == "rs" {
                            let rel = format!("src/{}/{}", name_str, sub.file_name().to_string_lossy());
                            files.push(rel);
                            if let Ok(content) = std::fs::read_to_string(&sub_path) {
                                loc += content.lines().count();
                                // Extract dependency hints
                                for line in content.lines() {
                                    if line.starts_with("use ") && !line.contains("::") {
                                        let dep = line
                                            .trim_start_matches("use ")
                                            .trim_end_matches(';')
                                            .split("::")
                                            .next()
                                            .unwrap_or("")
                                            .to_string();
                                        if !dep.is_empty() && dep != "crate" && dep != "super" && dep != "std" {
                                            if !deps.contains(&dep) {
                                                deps.push(dep);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            modules.push(ModuleProposal {
                name: name_str.clone(),
                description: format!("{} submodule from {}", name_str, url),
                files,
                loc_estimate: loc,
                dependencies: deps,
            });
        } else if let Some(ext) = path.extension() {
            if ext == "rs" && name_str != "main.rs" && name_str != "lib.rs" {
                let loc = std::fs::read_to_string(&path)
                    .map(|c| c.lines().count())
                    .unwrap_or(0);

                let mut deps = Vec::new();
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for line in content.lines() {
                        if line.starts_with("use ") {
                            let dep = line
                                .trim_start_matches("use ")
                                .trim_end_matches(';')
                                .split("::")
                                .next()
                                .unwrap_or("")
                                .to_string();
                            if !dep.is_empty() && dep != "crate" && dep != "super" && dep != "std" {
                                if !deps.contains(&dep) {
                                    deps.push(dep);
                                }
                            }
                        }
                    }
                }

                let clean_name = name_str.trim_end_matches(".rs").to_string();
                modules.push(ModuleProposal {
                    name: clean_name,
                    description: format!("{} module from {}", clean_name, url),
                    files: vec![format!("src/{}", name_str)],
                    loc_estimate: loc,
                    dependencies: deps,
                });
            }
        }
    }

    Ok(modules)
}

// ---------------------------------------------------------------------------
// Integration planning
// ---------------------------------------------------------------------------

/// Generate an integration plan for absorbing a repo into the terminal.
///
/// The plan includes new Cargo features, steps to integrate each module,
/// and an estimated effort.
pub fn suggest_integration(url: &str) -> IntegrationPlan {
    let analysis = analyze_repo(url).ok();
    let modules = decompose_modules(url).unwrap_or_default();

    let mut features = Vec::new();
    let mut steps = Vec::new();

    let repo_name = url
        .trim_end_matches(".git")
        .split('/')
        .last()
        .unwrap_or("unknown");

    // Feature for the whole module group
    features.push(FeatureEntry {
        name: format!("trending-{}", repo_name),
        dependencies: vec!["dep:tokio".to_string()],
        modules: modules.iter().map(|m| m.name.clone()).collect(),
        optional: true,
    });

    steps.push(IntegrationStep {
        step: 1,
        action: "Clone".to_string(),
        details: format!("git clone {} /tmp/repo-analysis", url),
    });

    if let Some(ref analysis) = analysis {
        steps.push(IntegrationStep {
            step: 2,
            action: "Analyze build system".to_string(),
            details: format!(
                "Language: {}, modules: {}, tests: {}",
                analysis.language, analysis.module_count, analysis.test_count
            ),
        });
    }

    // Per-module steps
    for (i, module) in modules.iter().enumerate() {
        steps.push(IntegrationStep {
            step: (i + 3) as u32,
            action: format!("Integrate module: {}", module.name),
            details: format!(
                "Copy src/{}.rs → terminal/src/{}.rs. Add Cargo feature. Wire into mod.rs. {} LOC.",
                module.name, module.name, module.loc_estimate
            ),
        });
    }

    steps.push(IntegrationStep {
        step: (modules.len() + 3) as u32,
        action: "Update Cargo.toml".to_string(),
        details: format!(
            "Add [features] entry for trending-{} with gated dependencies.",
            repo_name
        ),
    });

    steps.push(IntegrationStep {
        step: (modules.len() + 4) as u32,
        action: "Test".to_string(),
        details: "Run cargo build --features <new-feature> and cargo test.".to_string(),
    });

    let effort = (modules.len() * 10 + 15) as u32;

    IntegrationPlan {
        source_url: url.to_string(),
        new_features: features,
        steps,
        estimated_effort_minutes: effort,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn urlencoding(input: &str) -> String {
    input
        .chars()
        .flat_map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => {
                vec![c]
            }
            _ => {
                let bytes = c.to_string().into_bytes();
                bytes
                    .into_iter()
                    .flat_map(|b| format!("%{:02X}", b).chars().collect::<Vec<_>>())
                    .collect()
            }
        })
        .collect()
}

fn extract_toml_value(content: &str, key: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("{} = ", key)) {
            return trimmed
                .splitn(2, '=')
                .nth(1)
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_string();
        }
    }
    String::new()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urlencoding_basic() {
        assert_eq!(urlencoding("hello"), "hello");
        assert_eq!(urlencoding("hello world"), "hello%20world");
        assert_eq!(urlencoding("rust+wasm"), "rust%2Bwasm");
    }

    #[test]
    fn test_extract_toml_value() {
        let toml = r#"
[package]
name = "test"
version = "0.1.0"
edition = "2021"
"#;
        assert_eq!(extract_toml_value(toml, "name"), "test");
        assert_eq!(extract_toml_value(toml, "version"), "0.1.0");
        assert_eq!(extract_toml_value(toml, "edition"), "2021");
        assert_eq!(extract_toml_value(toml, "nonexistent"), "");
    }

    #[test]
    fn test_analyze_local_self() {
        // Analyze the wta crate itself
        let crate_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let analysis = analyze_local(&crate_path, "self-test").unwrap();
        assert_eq!(analysis.language, "Rust");
        assert!(analysis.has_build_system);
        assert!(analysis.module_count > 0);
    }

    #[test]
    fn test_decompose_local_self() {
        let crate_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let modules = decompose_local(&crate_path, "self-test").unwrap();
        // Should find some modules in the src/ directory
        if !modules.is_empty() {
            for m in &modules {
                assert!(!m.name.is_empty());
                assert!(m.loc_estimate > 0);
            }
        }
    }

    #[test]
    fn test_suggest_integration_returns_plan() {
        let plan = suggest_integration("https://github.com/example/test-repo.git");
        assert!(!plan.new_features.is_empty());
        assert!(!plan.steps.is_empty());
        assert!(plan.estimated_effort_minutes > 0);
    }
}
