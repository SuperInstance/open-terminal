//! Verification entropy status bar — always visible when `math-tools` is enabled.
//!
//! Tracks the edit-to-test ratio and computes Shannon-style verification
//! entropy: `S = -p_edit·ln(p_edit) - p_test·ln(p_test)`. Maximum entropy
//! (perfectly balanced) is `S_max = ln(2) ≈ 0.693`. The bar turns from green
//! (balanced) through yellow to red (dangerous — lots of edits, no tests).
//!
//! Design principle: the bar is a battery indicator for code quality. It never
//! sleeps, so you can't ignore verification debt.

use std::time::{Duration, Instant};

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

// ── Command classification ────────────────────────────────────────────────

/// Known edit commands (commands that change source code or artifacts).
const EDIT_COMMANDS: &[&str] = &[
    "vim",
    "vi",
    "nano",
    "emacs",
    "code",
    "cargo build",
    "cargo run",
    "cargo check",
    "cargo clippy",
    "cargo fix",
    "cargo add",
    "cargo rm",
    "make",
    "npm run build",
    "npm install",
    "npm ci",
    "yarn build",
    "yarn install",
    "pnpm build",
    "pnpm install",
    "go build",
    "go mod tidy",
    "go mod edit",
    "gcc",
    "g++",
    "clang",
    "pip install",
    "pip3 install",
    "uv pip install",
    "uv sync",
    "poetry install",
    "bundle install",
    "rails generate",
    "rails db:migrate",
    "kubectl apply",
    "terraform apply",
    "docker build",
    "git commit",
    "git add",
    "git push",
    "git merge",
    "git rebase",
    "sed",
    "awk",
    "patch",
];

/// Known test commands (commands that verify correctness).
const TEST_COMMANDS: &[&str] = &[
    "cargo test",
    "cargo nextest",
    "cargo bench",
    "pytest",
    "py.test",
    "python -m pytest",
    "python3 -m pytest",
    "npm test",
    "npm run test",
    "yarn test",
    "pnpm test",
    "jest",
    "vitest",
    "go test",
    "bundle exec rspec",
    "bundle exec cucumber",
    "rails test",
    "rails spec",
    "make test",
    "mvn test",
    "gradle test",
    "dotnet test",
    "zig build test",
    "swift test",
    "busted",
    "luassert",
    "ctest",
];

/// Command classification result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Edit,
    Test,
    Unknown,
}

/// Classify a shell command string as edit, test, or unknown.
pub fn classify_command(cmd: &str) -> CommandKind {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return CommandKind::Unknown;
    }
    // Extract the leading tokens (command + subcommand, e.g. "cargo test").
    let prefix = trimmed.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    let prefix_lower = prefix.to_lowercase();

    // Check test commands first (more specific, e.g. "cargo test" before "cargo").
    if TEST_COMMANDS.iter().any(|t| prefix_lower == *t) {
        return CommandKind::Test;
    }
    if EDIT_COMMANDS.iter().any(|e| prefix_lower == *e) {
        return CommandKind::Edit;
    }
    CommandKind::Unknown
}

// ── Entropy tracker ───────────────────────────────────────────────────────

/// Natural log of 2: maximum verification entropy for a binary edit/test split.
pub const S_MAX: f64 = std::f64::consts::LN_2; // ≈ 0.6931

/// Entropy threshold below which the bar turns red (dangerous).
pub const ENTROPY_DANGER: f64 = 0.1;
/// Entropy threshold for yellow (caution).
pub const ENTROPY_CAUTION: f64 = 0.3;
/// Number of consecutive edits before we emit a warning notification.
pub const UNTILED_EDIT_THRESHOLD: u32 = 20;

/// Tracks edit/test counts and computes verification entropy.
#[derive(Debug, Clone)]
pub struct EntropyTracker {
    edit_count: u32,
    test_count: u32,
    last_test_instant: Option<Instant>,
    consecutive_edits: u32,
    warning_emitted: bool,
}

impl Default for EntropyTracker {
    fn default() -> Self {
        Self {
            edit_count: 0,
            test_count: 0,
            last_test_instant: None,
            consecutive_edits: 0,
            warning_emitted: false,
        }
    }
}

impl EntropyTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a classified command.
    pub fn record(&mut self, kind: CommandKind) {
        match kind {
            CommandKind::Edit => {
                self.edit_count += 1;
                self.consecutive_edits += 1;
                if self.consecutive_edits >= UNTILED_EDIT_THRESHOLD && !self.warning_emitted {
                    self.warning_emitted = true;
                }
            }
            CommandKind::Test => {
                self.test_count += 1;
                self.consecutive_edits = 0;
                self.last_test_instant = Some(Instant::now());
                self.warning_emitted = false;
            }
            CommandKind::Unknown => {}
        }
    }

    /// Record a raw command string (convenience: classifies then records).
    pub fn record_command(&mut self, cmd: &str) {
        self.record(classify_command(cmd));
    }

    /// Compute verification entropy: S = -p_e·ln(p_e) - p_t·ln(p_t).
    /// Returns 0.0 if no commands have been recorded.
    pub fn entropy(&self) -> f64 {
        let total = self.total();
        if total == 0 || self.edit_count == 0 || self.test_count == 0 {
            return 0.0;
        }
        let p_e = self.edit_count as f64 / total as f64;
        let p_t = self.test_count as f64 / total as f64;
        -p_e * p_e.ln() - p_t * p_t.ln()
    }

    /// Normalized entropy: S / S_max ∈ [0, 1].
    pub fn normalized_entropy(&self) -> f64 {
        let s = self.entropy();
        if s <= 0.0 { 0.0 } else { s / S_MAX }
    }

    pub fn edit_count(&self) -> u32 {
        self.edit_count
    }

    pub fn test_count(&self) -> u32 {
        self.test_count
    }

    pub fn total(&self) -> u32 {
        self.edit_count + self.test_count
    }

    pub fn consecutive_edits(&self) -> u32 {
        self.consecutive_edits
    }

    /// How long since the last test command.
    pub fn time_since_last_test(&self) -> Option<Duration> {
        self.last_test_instant.map(|t| t.elapsed())
    }

    /// Whether a warning notification should be displayed.
    pub fn should_warn(&self) -> bool {
        self.warning_emitted
    }

    /// Acknowledge the warning (so it doesn't re-fire until conditions are met again).
    pub fn acknowledge_warning(&mut self) {
        self.warning_emitted = false;
    }

    /// Build the "Untested for X min" label.
    fn untested_label(&self) -> String {
        match self.time_since_last_test() {
            None if self.edit_count > 0 => format!("Never tested"),
            None => String::new(),
            Some(d) => {
                let mins = d.as_secs() / 60;
                if mins < 1 {
                    String::from("Just tested")
                } else if mins < 60 {
                    format!("Untested for {} min", mins)
                } else {
                    let hours = mins / 60;
                    format!("Untested for {}h {}m", hours, mins % 60)
                }
            }
        }
    }

    /// Generate the warning message if conditions are met.
    pub fn warning_message(&self) -> Option<String> {
        if self.consecutive_edits >= UNTILED_EDIT_THRESHOLD {
            Some(format!(
                "⚠️ Verification entropy conservation: you've edited {} commands without testing. \
                 Bugs are accumulating deterministically.",
                self.consecutive_edits
            ))
        } else {
            None
        }
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────

/// Color for the entropy bar based on current entropy level.
fn entropy_color(entropy: f64) -> Color {
    if entropy > ENTROPY_CAUTION {
        // Green zone
        Color::Green
    } else if entropy > ENTROPY_DANGER {
        // Yellow zone
        Color::Yellow
    } else {
        // Red zone
        Color::Red
    }
}

/// Build the bar graph string: filled blocks and empty blocks.
fn build_bar(entropy: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let ratio = if entropy <= 0.0 {
        0.0
    } else {
        (entropy / S_MAX).min(1.0)
    };
    let filled = (ratio * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// Format the compact status bar line.
pub fn format_status_line(tracker: &EntropyTracker, bar_width: usize) -> String {
    let entropy = tracker.entropy();
    let bar = build_bar(entropy, bar_width);
    let label = tracker.untested_label();
    format!(
        "{} S={:.2} │ {} edits, {} tests │ \"{}\"",
        bar,
        entropy,
        tracker.edit_count(),
        tracker.test_count(),
        label
    )
}

/// Render the entropy bar as a single-line status bar at the bottom of the
/// given area. Returns 1 (the height consumed).
pub fn render(frame: &mut Frame, area: Rect, tracker: &EntropyTracker) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let entropy = tracker.entropy();
    let color = entropy_color(entropy);
    let bar_visual_width = 12usize;
    let bar_str = build_bar(entropy, bar_visual_width);

    let label = tracker.untested_label();
    let counts = format!("{} edits, {} tests", tracker.edit_count(), tracker.test_count());
    let entropy_str = format!("S={:.2}", entropy);

    let mut spans: Vec<Span> = Vec::new();

    // Bar graph (colored)
    spans.push(Span::styled(
        bar_str,
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::raw(" "));

    // Entropy value
    spans.push(Span::styled(
        entropy_str,
        Style::default().fg(color),
    ));

    spans.push(Span::styled(
        " │ ",
        Style::default().fg(Color::DarkGray),
    ));

    // Edit/test counts
    spans.push(Span::styled(
        counts,
        Style::default().fg(Color::Gray),
    ));

    spans.push(Span::styled(
        " │ ",
        Style::default().fg(Color::DarkGray),
    ));

    // Untested label
    let label_color = if tracker.consecutive_edits() > 10 {
        Color::Red
    } else if tracker.time_since_last_test().map_or(false, |d| d.as_secs() > 600) {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    spans.push(Span::styled(
        format!("\"{}\"", label),
        Style::default().fg(label_color),
    ));

    // Warning indicator
    if tracker.should_warn() {
        spans.push(Span::styled(
            " ⚠",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(
        Style::default()
            .bg(Color::Rgb(20, 20, 20))
            .fg(Color::White),
    );
    frame.render_widget(paragraph, area);
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── classify_command ──────────────────────────────────────────────────

    #[test]
    fn classify_known_edit_commands() {
        assert_eq!(classify_command("cargo build"), CommandKind::Edit);
        assert_eq!(classify_command("cargo build --release"), CommandKind::Edit);
        assert_eq!(classify_command("vim main.rs"), CommandKind::Edit);
        assert_eq!(classify_command("make"), CommandKind::Edit);
        assert_eq!(classify_command("npm run build"), CommandKind::Edit);
        assert_eq!(classify_command("git commit -m \"feat\""), CommandKind::Edit);
    }

    #[test]
    fn classify_known_test_commands() {
        assert_eq!(classify_command("cargo test"), CommandKind::Test);
        assert_eq!(classify_command("cargo test --all"), CommandKind::Test);
        assert_eq!(classify_command("pytest"), CommandKind::Test);
        assert_eq!(classify_command("pytest tests/"), CommandKind::Test);
        assert_eq!(classify_command("npm test"), CommandKind::Test);
        assert_eq!(classify_command("go test ./..."), CommandKind::Test);
    }

    #[test]
    fn classify_unknown_commands() {
        assert_eq!(classify_command("ls"), CommandKind::Unknown);
        assert_eq!(classify_command("echo hello"), CommandKind::Unknown);
        assert_eq!(classify_command("cd /tmp"), CommandKind::Unknown);
        assert_eq!(classify_command(""), CommandKind::Unknown);
        assert_eq!(classify_command("   "), CommandKind::Unknown);
    }

    #[test]
    fn classify_is_case_insensitive() {
        assert_eq!(classify_command("CARGO TEST"), CommandKind::Test);
        assert_eq!(classify_command("Cargo Build"), CommandKind::Edit);
        assert_eq!(classify_command("VIM file.txt"), CommandKind::Edit);
    }

    #[test]
    fn test_commands_take_priority_over_edit_prefix() {
        // "cargo test" should be Test, not caught by "cargo build" Edit
        assert_eq!(classify_command("cargo test"), CommandKind::Test);
        // "cargo bench" is also Test
        assert_eq!(classify_command("cargo bench"), CommandKind::Test);
    }

    // ── EntropyTracker basics ─────────────────────────────────────────────

    #[test]
    fn empty_tracker_has_zero_entropy() {
        let t = EntropyTracker::new();
        assert_eq!(t.entropy(), 0.0);
        assert_eq!(t.edit_count(), 0);
        assert_eq!(t.test_count(), 0);
        assert_eq!(t.total(), 0);
    }

    #[test]
    fn only_edits_zero_entropy() {
        let mut t = EntropyTracker::new();
        t.record(CommandKind::Edit);
        t.record(CommandKind::Edit);
        t.record(CommandKind::Edit);
        assert_eq!(t.entropy(), 0.0);
        assert_eq!(t.edit_count(), 3);
    }

    #[test]
    fn only_tests_zero_entropy() {
        let mut t = EntropyTracker::new();
        t.record(CommandKind::Test);
        t.record(CommandKind::Test);
        assert_eq!(t.entropy(), 0.0);
        assert_eq!(t.test_count(), 2);
    }

    #[test]
    fn balanced_commands_max_entropy() {
        let mut t = EntropyTracker::new();
        for _ in 0..50 {
            t.record(CommandKind::Edit);
        }
        for _ in 0..50 {
            t.record(CommandKind::Test);
        }
        let s = t.entropy();
        // S should be very close to ln(2)
        assert!((s - S_MAX).abs() < 1e-9, "S={}, expected ≈ {}", s, S_MAX);
        assert!((t.normalized_entropy() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn skewed_entropy_between_zero_and_max() {
        let mut t = EntropyTracker::new();
        t.record(CommandKind::Edit);
        t.record(CommandKind::Edit);
        t.record(CommandKind::Edit);
        t.record(CommandKind::Test);
        let s = t.entropy();
        assert!(s > 0.0 && s < S_MAX, "S={}", s);
    }

    #[test]
    fn unknown_commands_are_ignored() {
        let mut t = EntropyTracker::new();
        t.record(CommandKind::Unknown);
        t.record(CommandKind::Unknown);
        assert_eq!(t.total(), 0);
        assert_eq!(t.entropy(), 0.0);
    }

    #[test]
    fn record_command_convenience_method() {
        let mut t = EntropyTracker::new();
        t.record_command("cargo build");
        t.record_command("cargo test");
        assert_eq!(t.edit_count(), 1);
        assert_eq!(t.test_count(), 1);
        assert!((t.entropy() - S_MAX).abs() < 1e-9);
    }

    // ── Warning system ────────────────────────────────────────────────────

    #[test]
    fn warning_fires_after_threshold_edits() {
        let mut t = EntropyTracker::new();
        for _ in 0..UNTILED_EDIT_THRESHOLD {
            t.record(CommandKind::Edit);
        }
        assert!(t.should_warn());
        assert!(t.warning_message().is_some());
    }

    #[test]
    fn warning_clears_on_test() {
        let mut t = EntropyTracker::new();
        for _ in 0..UNTILED_EDIT_THRESHOLD {
            t.record(CommandKind::Edit);
        }
        assert!(t.should_warn());
        t.record(CommandKind::Test);
        assert!(!t.should_warn());
        assert_eq!(t.consecutive_edits(), 0);
    }

    #[test]
    fn warning_acknowledged() {
        let mut t = EntropyTracker::new();
        for _ in 0..UNTILED_EDIT_THRESHOLD {
            t.record(CommandKind::Edit);
        }
        assert!(t.should_warn());
        t.acknowledge_warning();
        assert!(!t.should_warn());
    }

    #[test]
    fn warning_no_fire_below_threshold() {
        let mut t = EntropyTracker::new();
        for _ in 0..UNTILED_EDIT_THRESHOLD - 1 {
            t.record(CommandKind::Edit);
        }
        assert!(!t.should_warn());
    }

    // ── Color mapping ─────────────────────────────────────────────────────

    #[test]
    fn color_green_for_balanced() {
        // S > 0.3 → green
        assert_eq!(entropy_color(0.5), Color::Green);
        assert_eq!(entropy_color(S_MAX), Color::Green);
    }

    #[test]
    fn color_yellow_for_caution() {
        // 0.1 < S ≤ 0.3 → yellow
        assert_eq!(entropy_color(0.2), Color::Yellow);
        assert_eq!(entropy_color(0.15), Color::Yellow);
    }

    #[test]
    fn color_red_for_dangerous() {
        // S ≤ 0.1 → red
        assert_eq!(entropy_color(0.05), Color::Red);
        assert_eq!(entropy_color(0.0), Color::Red);
    }

    // ── Bar string ────────────────────────────────────────────────────────

    #[test]
    fn bar_full_at_max_entropy() {
        let bar = build_bar(S_MAX, 12);
        assert_eq!(bar, "████████████");
    }

    #[test]
    fn bar_empty_at_zero_entropy() {
        let bar = build_bar(0.0, 12);
        assert_eq!(bar, "░░░░░░░░░░░░");
    }

    #[test]
    fn bar_partial_at_half_entropy() {
        // S ≈ S_MAX/2 → roughly half filled
        let bar = build_bar(S_MAX / 2.0, 12);
        let filled = bar.chars().filter(|c| *c == '█').count();
        assert_eq!(filled, 6);
    }

    #[test]
    fn bar_zero_width_empty() {
        let bar = build_bar(S_MAX, 0);
        assert_eq!(bar, "");
    }

    // ── Status line formatting ────────────────────────────────────────────

    #[test]
    fn status_line_format() {
        let mut t = EntropyTracker::new();
        t.record(CommandKind::Edit);
        t.record(CommandKind::Edit);
        t.record(CommandKind::Test);
        let line = format_status_line(&t, 12);
        assert!(line.contains("S=0.92"), "expected S≈0.92, got: {}", line);
        assert!(line.contains("2 edits"));
        assert!(line.contains("1 tests"));
    }

    // ── Normalized entropy ────────────────────────────────────────────────

    #[test]
    fn normalized_entropy_bounds() {
        let mut t = EntropyTracker::new();
        assert_eq!(t.normalized_entropy(), 0.0);
        t.record(CommandKind::Edit);
        t.record(CommandKind::Test);
        assert!((t.normalized_entropy() - 1.0).abs() < 1e-9);
    }

    // ── Edge cases ────────────────────────────────────────────────────────

    #[test]
    fn entropy_symmetry_edit_test() {
        // Order shouldn't matter for final entropy value
        let mut t1 = EntropyTracker::new();
        t1.record(CommandKind::Edit);
        t1.record(CommandKind::Edit);
        t1.record(CommandKind::Test);

        let mut t2 = EntropyTracker::new();
        t2.record(CommandKind::Test);
        t2.record(CommandKind::Edit);
        t2.record(CommandKind::Edit);

        assert!((t1.entropy() - t2.entropy()).abs() < 1e-12);
    }

    #[test]
    fn s_max_is_ln2() {
        assert!((S_MAX - std::f64::consts::LN_2).abs() < 1e-15);
    }

    #[test]
    fn untested_label_when_never_tested() {
        let mut t = EntropyTracker::new();
        t.record(CommandKind::Edit);
        let label = t.untested_label();
        assert_eq!(label, "Never tested");
    }

    #[test]
    fn untested_label_when_no_commands() {
        let t = EntropyTracker::new();
        let label = t.untested_label();
        assert_eq!(label, "");
    }
}
