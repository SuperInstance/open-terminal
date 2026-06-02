//! Context-aware compression and alias suggestion (Adinkra symbols).
//!
//! Detects project type from files present:
//!   Cargo.toml → Rust
//!   package.json → Node
//!
//! Suggests context-appropriate aliases:
//!   "In this project, `cb` could mean `cargo build`"
//!
//! Tracks which aliases are actually used (cultural recoverability):
//!   the adinkra metaphor — symbols that are recognized carry meaning.

use std::collections::HashMap;

/// A detected project type with its associated command patterns.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectContext {
    /// The detected project type.
    pub kind: ProjectKind,
    /// The project file that triggered detection.
    pub trigger_file: String,
    /// Suggested aliases for this project.
    pub suggested_aliases: Vec<AliasSuggestion>,
}

/// Known project types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectKind {
    Rust,
    Node,
    Python,
    Go,
    Ruby,
    Java,
    Unknown,
}

impl std::fmt::Display for ProjectKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectKind::Rust => write!(f, "Rust"),
            ProjectKind::Node => write!(f, "Node.js"),
            ProjectKind::Python => write!(f, "Python"),
            ProjectKind::Go => write!(f, "Go"),
            ProjectKind::Ruby => write!(f, "Ruby"),
            ProjectKind::Java => write!(f, "Java"),
            ProjectKind::Unknown => write!(f, "Unknown"),
        }
    }
}

/// A suggested alias for a command.
#[derive(Debug, Clone, PartialEq)]
pub struct AliasSuggestion {
    /// Short alias string.
    pub alias: String,
    /// The command it expands to.
    pub expansion: String,
    /// Description of what the alias does.
    pub description: String,
    /// Whether this alias has been "culturally recovered" (used at least once).
    pub adopted: bool,
}

/// The adinkra compressor: detects project context and suggests aliases.
#[derive(Debug, Clone, Default)]
pub struct AdinkraCompressor {
    /// Track adopted aliases: alias → usage count.
    adopted_aliases: HashMap<String, u32>,
}

impl AdinkraCompressor {
    /// Create a new compressor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Detect the project context from a list of files present in the directory.
    pub fn detect_project(files: &[&str]) -> Option<ProjectContext> {
        let file_set: std::collections::HashSet<&str> = files.iter().copied().collect();

        let (kind, trigger) = if file_set.contains("Cargo.toml") {
            (ProjectKind::Rust, "Cargo.toml")
        } else if file_set.contains("package.json") {
            (ProjectKind::Node, "package.json")
        } else if file_set.contains("setup.py") || file_set.contains("pyproject.toml") || file_set.contains("requirements.txt") {
            (ProjectKind::Python, if file_set.contains("pyproject.toml") { "pyproject.toml" } else if file_set.contains("setup.py") { "setup.py" } else { "requirements.txt" })
        } else if file_set.contains("go.mod") {
            (ProjectKind::Go, "go.mod")
        } else if file_set.contains("Gemfile") {
            (ProjectKind::Ruby, "Gemfile")
        } else if file_set.contains("pom.xml") || file_set.contains("build.gradle") {
            (ProjectKind::Java, if file_set.contains("pom.xml") { "pom.xml" } else { "build.gradle" })
        } else {
            return None;
        };

        let suggested_aliases = Self::default_aliases_for(kind);

        Some(ProjectContext {
            kind,
            trigger_file: trigger.to_string(),
            suggested_aliases,
        })
    }

    /// Generate default alias suggestions for a project type.
    pub fn default_aliases_for(kind: ProjectKind) -> Vec<AliasSuggestion> {
        match kind {
            ProjectKind::Rust => vec![
                AliasSuggestion { alias: "cb".into(), expansion: "cargo build".into(), description: "Build the Rust project".into(), adopted: false },
                AliasSuggestion { alias: "ct".into(), expansion: "cargo test".into(), description: "Run tests".into(), adopted: false },
                AliasSuggestion { alias: "cr".into(), expansion: "cargo run".into(), description: "Run the project".into(), adopted: false },
                AliasSuggestion { alias: "cc".into(), expansion: "cargo check".into(), description: "Check for compilation errors".into(), adopted: false },
                AliasSuggestion { alias: "cf".into(), expansion: "cargo fmt".into(), description: "Format code".into(), adopted: false },
                AliasSuggestion { alias: "cl".into(), expansion: "cargo clippy".into(), description: "Run linter".into(), adopted: false },
            ],
            ProjectKind::Node => vec![
                AliasSuggestion { alias: "ni".into(), expansion: "npm install".into(), description: "Install dependencies".into(), adopted: false },
                AliasSuggestion { alias: "nr".into(), expansion: "npm run".into(), description: "Run npm script".into(), adopted: false },
                AliasSuggestion { alias: "nt".into(), expansion: "npm test".into(), description: "Run tests".into(), adopted: false },
                AliasSuggestion { alias: "ns".into(), expansion: "npm start".into(), description: "Start the project".into(), adopted: false },
                AliasSuggestion { alias: "nb".into(), expansion: "npm run build".into(), description: "Build the project".into(), adopted: false },
            ],
            ProjectKind::Python => vec![
                AliasSuggestion { alias: "pi".into(), expansion: "pip install".into(), description: "Install packages".into(), adopted: false },
                AliasSuggestion { alias: "pt".into(), expansion: "pytest".into(), description: "Run tests".into(), adopted: false },
                AliasSuggestion { alias: "pr".into(), expansion: "python".into(), description: "Run Python".into(), adopted: false },
            ],
            ProjectKind::Go => vec![
                AliasSuggestion { alias: "gb".into(), expansion: "go build".into(), description: "Build the project".into(), adopted: false },
                AliasSuggestion { alias: "gt".into(), expansion: "go test".into(), description: "Run tests".into(), adopted: false },
                AliasSuggestion { alias: "gr".into(), expansion: "go run".into(), description: "Run the project".into(), adopted: false },
            ],
            ProjectKind::Ruby => vec![
                AliasSuggestion { alias: "rb".into(), expansion: "bundle exec".into(), description: "Run with bundler".into(), adopted: false },
                AliasSuggestion { alias: "rt".into(), expansion: "bundle exec rspec".into(), description: "Run tests".into(), adopted: false },
            ],
            ProjectKind::Java => vec![
                AliasSuggestion { alias: "jb".into(), expansion: "mvn compile".into(), description: "Compile with Maven".into(), adopted: false },
                AliasSuggestion { alias: "jt".into(), expansion: "mvn test".into(), description: "Run tests".into(), adopted: false },
            ],
            ProjectKind::Unknown => vec![],
        }
    }

    /// Mark an alias as adopted (used by the user).
    pub fn adopt_alias(&mut self, alias: &str) {
        *self.adopted_aliases.entry(alias.to_string()).or_insert(0) += 1;
    }

    /// Check if an alias has been adopted.
    pub fn is_adopted(&self, alias: &str) -> bool {
        self.adopted_aliases.contains_key(alias)
    }

    /// Get adoption count for an alias.
    pub fn adoption_count(&self, alias: &str) -> u32 {
        *self.adopted_aliases.get(alias).unwrap_or(&0)
    }

    /// Update suggestions based on actual usage data.
    /// Moves adopted aliases to the front and sorts by usage.
    pub fn rank_suggestions(&self, suggestions: &mut [AliasSuggestion]) {
        for s in suggestions.iter_mut() {
            s.adopted = self.is_adopted(&s.alias);
        }
        suggestions.sort_by(|a, b| {
            let a_count = self.adoption_count(&a.alias);
            let b_count = self.adoption_count(&b.alias);
            b_count.cmp(&a_count)
        });
    }

    /// Detect which commands from a history match known project aliases.
    /// Returns (alias, command) pairs for commands that could be compressed.
    pub fn compress_commands(
        &self,
        commands: &[String],
        context: &ProjectContext,
    ) -> Vec<(String, String)> {
        let mut compressed = Vec::new();
        for cmd in commands {
            for suggestion in &context.suggested_aliases {
                if cmd == &suggestion.expansion {
                    compressed.push((suggestion.alias.clone(), cmd.clone()));
                    break;
                }
            }
        }
        compressed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_rust_project() {
        let ctx = AdinkraCompressor::detect_project(&["Cargo.toml", "src/main.rs"]).unwrap();
        assert_eq!(ctx.kind, ProjectKind::Rust);
        assert_eq!(ctx.trigger_file, "Cargo.toml");
        assert!(!ctx.suggested_aliases.is_empty());
    }

    #[test]
    fn detect_node_project() {
        let ctx = AdinkraCompressor::detect_project(&["package.json", "index.js"]).unwrap();
        assert_eq!(ctx.kind, ProjectKind::Node);
    }

    #[test]
    fn detect_python_project_pyproject() {
        let ctx = AdinkraCompressor::detect_project(&["pyproject.toml"]).unwrap();
        assert_eq!(ctx.kind, ProjectKind::Python);
    }

    #[test]
    fn detect_python_project_setup() {
        let ctx = AdinkraCompressor::detect_project(&["setup.py"]).unwrap();
        assert_eq!(ctx.kind, ProjectKind::Python);
    }

    #[test]
    fn detect_python_project_requirements() {
        let ctx = AdinkraCompressor::detect_project(&["requirements.txt"]).unwrap();
        assert_eq!(ctx.kind, ProjectKind::Python);
    }

    #[test]
    fn detect_go_project() {
        let ctx = AdinkraCompressor::detect_project(&["go.mod"]).unwrap();
        assert_eq!(ctx.kind, ProjectKind::Go);
    }

    #[test]
    fn detect_ruby_project() {
        let ctx = AdinkraCompressor::detect_project(&["Gemfile"]).unwrap();
        assert_eq!(ctx.kind, ProjectKind::Ruby);
    }

    #[test]
    fn detect_java_project() {
        let ctx = AdinkraCompressor::detect_project(&["pom.xml"]).unwrap();
        assert_eq!(ctx.kind, ProjectKind::Java);
    }

    #[test]
    fn detect_java_gradle() {
        let ctx = AdinkraCompressor::detect_project(&["build.gradle"]).unwrap();
        assert_eq!(ctx.kind, ProjectKind::Java);
        assert_eq!(ctx.trigger_file, "build.gradle");
    }

    #[test]
    fn no_detection_empty() {
        assert!(AdinkraCompressor::detect_project(&[]).is_none());
    }

    #[test]
    fn no_detection_unknown() {
        assert!(AdinkraCompressor::detect_project(&["Makefile", "README.md"]).is_none());
    }

    #[test]
    fn rust_aliases() {
        let aliases = AdinkraCompressor::default_aliases_for(ProjectKind::Rust);
        assert!(aliases.iter().any(|a| a.alias == "cb" && a.expansion == "cargo build"));
        assert!(aliases.iter().any(|a| a.alias == "ct" && a.expansion == "cargo test"));
    }

    #[test]
    fn node_aliases() {
        let aliases = AdinkraCompressor::default_aliases_for(ProjectKind::Node);
        assert!(aliases.iter().any(|a| a.alias == "ni" && a.expansion == "npm install"));
    }

    #[test]
    fn adopt_alias() {
        let mut comp = AdinkraCompressor::new();
        assert!(!comp.is_adopted("cb"));
        comp.adopt_alias("cb");
        assert!(comp.is_adopted("cb"));
        assert_eq!(comp.adoption_count("cb"), 1);
        comp.adopt_alias("cb");
        assert_eq!(comp.adoption_count("cb"), 2);
    }

    #[test]
    fn rank_suggestions_by_usage() {
        let mut comp = AdinkraCompressor::new();
        comp.adopt_alias("ct");
        comp.adopt_alias("ct");
        comp.adopt_alias("cb");

        let mut suggestions = AdinkraCompressor::default_aliases_for(ProjectKind::Rust);
        comp.rank_suggestions(&mut suggestions);
        // ct (count=2) should be first, cb (count=1) second
        assert_eq!(suggestions[0].alias, "ct");
        assert_eq!(suggestions[1].alias, "cb");
        assert!(suggestions[0].adopted);
    }

    #[test]
    fn compress_commands_rust() {
        let comp = AdinkraCompressor::new();
        let ctx = AdinkraCompressor::detect_project(&["Cargo.toml"]).unwrap();
        let commands = vec!["cargo build".into(), "cargo test".into(), "ls".into()];
        let compressed = comp.compress_commands(&commands, &ctx);
        assert_eq!(compressed.len(), 2);
        assert_eq!(compressed[0], ("cb".into(), "cargo build".into()));
        assert_eq!(compressed[1], ("ct".into(), "cargo test".into()));
    }

    #[test]
    fn project_kind_display() {
        assert_eq!(ProjectKind::Rust.to_string(), "Rust");
        assert_eq!(ProjectKind::Node.to_string(), "Node.js");
        assert_eq!(ProjectKind::Unknown.to_string(), "Unknown");
    }
}
