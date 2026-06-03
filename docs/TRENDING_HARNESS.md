# Trending Repo Harness — The Terminal as Universal Decomposer

> *Every GitHub repo is a potential terminal module.*

The **Trending Repo Harness** gives the Intelligent Terminal the ability to
find, clone, analyze, decompose, and absorb any GitHub repository. It
treats open-source code as a distributed module ecosystem — and the
terminal as the universal decomposer that extracts value from it.

## Philosophy

Most terminals consume code. This one **digests** it.

When you find an interesting Rust crate, a CLI tool, or a TUI application
on GitHub, the trending harness can:

1. **Clone** it in seconds (`git clone --depth 1`)
2. **Analyze** its structure — language, build system, module count, test
   coverage, CI configuration, and detected patterns (async, CLI, TUI, WASM)
3. **Decompose** it into independent module proposals — each one a natural
   unit extracted from the source tree
4. **Plan integration** — a step-by-step plan to add those modules as new
   terminal features, complete with Cargo feature gates and wiring

This isn't "copy-paste." It's **absorption** — understanding a repo's
architecture and offering a surgical integration path into the terminal.

## Feature Gate

The harness is gated behind the `trending` Cargo feature:

```toml
cargo build --features trending
```

This keeps it optional — the terminal doesn't pull in `reqwest`, `chrono`,
or `tempfile` unless you're actively using the harness.

## Module Structure

The harness lives at `src/trending_harness.rs` and exports these key
types and functions:

### Types

```rust
/// A trending repository discovered via the GitHub Search API.
pub struct TrendingRepo {
    pub url: String,
    pub stars: u64,
    pub topics: Vec<String>,
    pub timestamp: DateTime<Utc>,
    pub full_name: String,
    pub description: String,
}

/// Structural analysis of a repo's source tree.
pub struct RepoAnalysis {
    pub url: String,
    pub language: String,
    pub module_count: usize,
    pub test_count: usize,
    pub ci_configs: Vec<String>,
    pub patterns: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub has_build_system: bool,
}

/// A proposed module extracted from a repo's source code.
pub struct ModuleProposal {
    pub name: String,
    pub description: String,
    pub files: Vec<String>,
    pub loc_estimate: usize,
    pub dependencies: Vec<String>,
}

/// An integration plan describing how to add external modules as terminal features.
pub struct IntegrationPlan {
    pub source_url: String,
    pub new_features: Vec<FeatureEntry>,
    pub steps: Vec<IntegrationStep>,
    pub estimated_effort_minutes: u32,
}
```

### Functions

| Function | Description |
|----------|-------------|
| `fetch_trending(topic, limit)` | Searches GitHub for repos matching a topic, sorted by stars |
| `analyze_repo(url)` | Clones and analyzes a repo's structure |
| `analyze_local(path, url)` | Analyzes an already-cloned local repo |
| `decompose_modules(url)` | Clones and decomposes a repo into module proposals |
| `decompose_local(path, url)` | Decomposes an already-cloned local repo |
| `suggest_integration(url)` | Generates a complete integration plan |

### Pattern Detection

The analyzer detects these patterns from build files and source code:

- **library** — contains `[lib]` in Cargo.toml
- **binary** — contains `[[bin]]` in Cargo.toml
- **wasm** — references wasm-bindgen or wasm-pack
- **async** — uses tokio or async-std
- **cli** — uses clap or structopt
- **tui** — uses ratatui, crossterm, or similar
- **machine-learning** — README mentions ML/neural concepts
- **api** — README mentions API
- **graph** — README mentions graphs

## Usage Examples

### Find trending terminal-related repos

```rust
use wta::trending_harness::fetch_trending;

let repos = fetch_trending("terminal", 5).unwrap();
for repo in &repos {
    println!("★ {} - {} stars — {}", repo.full_name, repo.stars, repo.description);
}
```

### Analyze a repo

```rust
use wta::trending_harness::analyze_repo;

let analysis = analyze_repo("https://github.com/ratatui-org/ratatui.git").unwrap();
println!("Language: {}", analysis.language);
println!("Modules: {}", analysis.module_count);
println!("Tests: {}", analysis.test_count);
println!("Patterns: {:?}", analysis.patterns);
```

### Decompose into modules

```rust
use wta::trending_harness::decompose_modules;

let modules = decompose_modules("https://github.com/example/some-crate.git").unwrap();
for module in &modules {
    println!("  {} — {} LOC, depends on {:?}", module.name, module.loc_estimate, module.dependencies);
}
```

### Generate an integration plan

```rust
use wta::trending_harness::suggest_integration;

let plan = suggest_integration("https://github.com/example/some-crate.git");
println!("Estimated effort: {} minutes", plan.estimated_effort_minutes);
for step in &plan.steps {
    println!("{}. {} — {}", step.step, step.action, step.details);
}
```

## How to Find Repos Worth Absorbing

### High-potential signals

- **Stars:** 100–5000 (active but not overwhelming)
- **Topics:** `terminal`, `tui`, `cli`, `rust`, `wasm`, `async`
- **Fresh code:** Last commit < 6 months
- **Testing:** At least a few test files
- **Module boundaries:** Clear separation of concerns in the source tree

### Suggested search topics

| Topic | What you'll find |
|-------|-----------------|
| `terminal` | Terminal emulators, multiplexers, shell tools |
| `tui` | Ratatui/Crossterm-based TUIs |
| `cli` | CLI frameworks and tools |
| `rust-cli` | Rust-specific CLI utilities |
| `async` | Async runtime tools |
| `wasm` | WebAssembly tools (absorb into wasm support) |
| `graph` | Graph algorithms (potential math-tools additions) |
| `machine-learning` | ML inference tools |

## Integration Pattern

When absorbing a repo as a terminal feature, follow this pattern:

1. **Create the module** — copy source files into `src/`
2. **Feature gate** — wrap module imports with `#[cfg(feature = "trending-<name>")]`
3. **Add Cargo feature** — add an entry in `[features]`
4. **Wire optional deps** — add `dep:` entries under `[features]`
5. **Test** — `cargo build --features trending,trending-<name>`
6. **Commit** — with a clear absorption message

### Example Cargo.toml additions

```toml
[features]
trending-ratatui-widgets = ["trending"]

[dependencies]
ratatui-custom-widgets = { git = "...", optional = true }
```

---

*The terminal doesn't just run code. It becomes what it consumes.*
