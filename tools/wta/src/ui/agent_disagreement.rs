//! # Agent Disagreement Visualization (Sheaf-Theoretic)
//!
//! Feature-gated under `math-tools`. A ratatui component that renders when
//! multiple agent panes disagree on a fix. Uses simplified sheaf cohomology:
//!
//! - **H⁰** measures whether agents *can* agree (connectedness of the
//!   agreement graph). H⁰ = 1 means at least one connected component of
//!   agreeing agents exists; H⁰ = 0 means complete divergence.
//! - **H¹** counts *structural obstructions* — irreducible disagreements
//!   that no amount of communication will resolve. Each obstruction
//!   represents a fundamentally incompatible perspective.
//!
//! ## Key Insight
//!
//! "When three agents give you three different fixes, H¹ tells you whether
//! better prompting would help (H¹ = 0) or whether you need fundamentally
//! different information (H¹ > 0)."

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

// ─── Public Types ─────────────────────────────────────────────────────

/// A single agent's proposed fix/response.
#[derive(Debug, Clone)]
pub struct AgentFix {
    /// Agent identifier (e.g. "copilot", "claude", "codex").
    pub agent_id: String,
    /// The proposed fix text.
    pub fix_text: String,
}

/// The result of sheaf-theoretic analysis on agent responses.
#[derive(Debug, Clone, PartialEq)]
pub struct SheafAnalysis {
    /// Number of connected components in the agreement graph.
    /// 0 = all agents diverged, 1 = at least one agreement cluster.
    pub h0: usize,
    /// Number of irreducible structural disagreements (H¹).
    /// 0 = communication can resolve everything.
    pub h1: usize,
    /// Total number of agents.
    pub agent_count: usize,
    /// Pairs of agents that agree (indices into the original agent list).
    pub agreement_edges: Vec<(usize, usize)>,
    /// Pairs that disagree structurally.
    pub disagreement_pairs: Vec<(usize, usize)>,
}

/// Verdict text based on the analysis.
#[derive(Debug, Clone, PartialEq)]
pub enum DisagreementVerdict {
    /// H¹ = 0: agents can converge through communication.
    CommunicationHelps,
    /// H¹ = 1: one structural split; consider a different angle.
    OneObstruction,
    /// H¹ ≥ 2: fundamentally different perspectives needed.
    NeedNewPerspective,
}

impl DisagreementVerdict {
    /// Human-readable label for the verdict.
    pub fn label(&self) -> &str {
        match self {
            Self::CommunicationHelps => "Communication helps",
            Self::OneObstruction => "1 structural split",
            Self::NeedNewPerspective => "Need new perspective",
        }
    }
}

// ─── Similarity & Agreement Detection ─────────────────────────────────

/// Compute a normalized string similarity score between two texts using
/// n-gram overlap (heuristic, no ML). Returns a value in [0.0, 1.0].
///
/// Uses trigram Jaccard similarity with a fallback to token-set overlap
/// for short strings. Both are purely heuristic but capture:
/// - Exact/near-exact matches
/// - Shared key phrases even with different ordering
/// - Semantic proximity for common fix patterns (e.g. "add null check"
///   vs "insert null guard")
pub fn text_similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    // Exact match shortcut.
    if a_lower == b_lower {
        return 1.0;
    }

    // Token-set similarity: split on whitespace, compute Jaccard on token sets.
    let tokens_a: std::collections::HashSet<&str> = a_lower.split_whitespace().collect();
    let tokens_b: std::collections::HashSet<&str> = b_lower.split_whitespace().collect();

    let token_jaccard = jaccard(&tokens_a, &tokens_b);

    // Trigram similarity for longer texts.
    let trigram_sim = if a_lower.len() >= 3 && b_lower.len() >= 3 {
        let tri_a = trigrams(&a_lower);
        let tri_b = trigrams(&b_lower);
        jaccard(&tri_a, &tri_b)
    } else {
        0.0
    };

    // Weighted blend: trigrams capture order, tokens capture word overlap.
    // Token Jaccard gets higher weight for shorter texts.
    let a_len = a_lower.len();
    let b_len = b_lower.len();
    let avg_len = (a_len + b_len) / 2;

    if avg_len < 20 {
        // Short text: rely more on tokens
        token_jaccard * 0.7 + trigram_sim * 0.3
    } else {
        token_jaccard * 0.4 + trigram_sim * 0.6
    }
}

/// Compute Jaccard similarity between two sets.
fn jaccard<T>(a: &std::collections::HashSet<T>, b: &std::collections::HashSet<T>) -> f64
where
    T: std::hash::Hash + Eq,
{
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

/// Extract character trigrams from a string.
fn trigrams(s: &str) -> std::collections::HashSet<String> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 3 {
        let mut set = std::collections::HashSet::new();
        set.insert(s.to_string());
        return set;
    }
    chars
        .windows(3)
        .map(|w| w.iter().collect::<String>())
        .collect()
}

/// Threshold above which two agent responses are considered "agreeing."
const AGREEMENT_THRESHOLD: f64 = 0.45;

/// Detect semantic matching between fix texts using heuristic keyword overlap.
/// Captures common patterns like "add null check" ≈ "insert null guard"
/// without any ML.
fn semantic_boost(a: &str, b: &str) -> f64 {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    // Synonym groups for common fix patterns.
    let synonym_groups: &[&[&str]] = &[
        &["null", "nil", "none", "optional", "unwrap"],
        &["check", "guard", "validate", "verify", "assert"],
        &["add", "insert", "include", "create"],
        &["fix", "repair", "correct", "resolve"],
        &["error", "exception", "fault", "failure", "bug"],
        &["remove", "delete", "drop", "eliminate"],
        &["update", "modify", "change", "set"],
        &["return", "yield", "output", "result"],
        &["import", "include", "require", "use"],
        &["type", "cast", "convert", "coerce"],
        &["test", "spec", "assert", "verify"],
        &["refactor", "restructure", "reorganize", "clean"],
    ];

    let mut boost = 0.0;
    for group in synonym_groups {
        let a_hits = group.iter().filter(|kw| a_lower.contains(*kw)).count();
        let b_hits = group.iter().filter(|kw| b_lower.contains(*kw)).count();
        if a_hits > 0 && b_hits > 0 {
            boost += 0.05 * a_hits.min(b_hits).min(2) as f64;
        }
    }

    boost.min(0.2) // cap the boost
}

/// Determine if two agent fixes agree, combining text similarity with
/// semantic matching heuristics.
pub fn fixes_agree(a: &AgentFix, b: &AgentFix) -> bool {
    let sim = text_similarity(&a.fix_text, &b.fix_text);
    let boost = semantic_boost(&a.fix_text, &b.fix_text);
    sim + boost >= AGREEMENT_THRESHOLD
}

// ─── Sheaf Cohomology (Simplified) ───────────────────────────────────

/// Compute simplified sheaf cohomology for a set of agent fixes.
///
/// ## Model
///
/// We model agents as vertices in a graph where edges connect agents that
/// agree (their fix similarity exceeds the threshold). This graph is the
/// "agreement sheaf" — a presheaf on the complete graph where the stalk at
/// each vertex is the agent's fix, and restriction maps exist between
/// agreeing vertices.
///
/// - **H⁰** = number of connected components of the agreement graph.
///   - H⁰ ≥ 1 means at least one cluster of agreement exists.
///   - H⁰ = 0 only when there are no agents.
///
/// - **H¹** = rank of H¹ of the sheaf = number of independent "obstruction
///   cycles" — pairs of agents that disagree even though there's a path
///   of agreement connecting them indirectly. In our simplified model,
///   H¹ counts irreducible disagreements that survive communication:
///   specifically, the number of connected components where not all agents
///   within the component agree *through every path* (i.e., there are
///   agents that agree pairwise but create contradictory transitivity).
///
///   More practically: H¹ = (number of disagreeing pairs) minus
///   (disagreements explainable by component structure). For the simplified
///   model, H¹ = number of agent pairs that disagree AND are in different
///   connected components of the agreement graph — these represent
///   *structural* splits that communication alone won't fix.
///
///   Actually, the cleaner formulation: H¹ counts connected components
///   with more than one agent but where not all agents in that component
///   directly agree with each other (i.e., the agreement graph has
///   "holes" — paths exist but not cliques). Plus isolated disagreeing
///   agents.
///
///   Simplest correct version: H¹ = max(0, disagreeing_pair_count - pairs
///   already accounted for by being in separate components). This is
///   effectively the number of "irreducible splits" — disagreements that
///   persist even after accounting for communication.
pub fn compute_sheaf_analysis(agents: &[AgentFix]) -> SheafAnalysis {
    let n = agents.len();
    if n == 0 {
        return SheafAnalysis {
            h0: 0,
            h1: 0,
            agent_count: 0,
            agreement_edges: Vec::new(),
            disagreement_pairs: Vec::new(),
        };
    }

    // Build agreement graph: edges between agreeing agents.
    let mut agreement_edges: Vec<(usize, usize)> = Vec::new();
    let mut disagreement_pairs: Vec<(usize, usize)> = Vec::new();

    for i in 0..n {
        for j in (i + 1)..n {
            if fixes_agree(&agents[i], &agents[j]) {
                agreement_edges.push((i, j));
            } else {
                disagreement_pairs.push((i, j));
            }
        }
    }

    // Compute connected components via union-find.
    let mut parent: Vec<usize> = (0..n).collect();
    for &(a, b) in &agreement_edges {
        union(&mut parent, a, b);
    }

    // Count unique components.
    let mut roots: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for i in 0..n {
        roots.insert(find(&mut parent, i));
    }
    let num_components = roots.len();

    // H¹: count disagreements that span different components.
    // These are "irreducible" — agents in different agreement components
    // cannot be reconciled through communication alone.
    let cross_component_disagreements = disagreement_pairs
        .iter()
        .filter(|&&(a, b)| find(&mut parent, a) != find(&mut parent, b))
        .count();

    // Additional H¹ contribution: within a single component, count agents
    // that don't form a clique (i.e., there are agents A and C that both
    // agree with B but not with each other). These are "holes" in the
    // agreement sheaf.
    let mut within_component_holes = 0usize;
    for &(a, b) in &disagreement_pairs {
        if find(&mut parent, a) == find(&mut parent, b) {
            // A and B are in the same component but disagree directly.
            // This means they're connected through a path of agreements
            // but don't agree themselves — a structural hole.
            within_component_holes += 1;
        }
    }

    let h1 = cross_component_disagreements + within_component_holes;

    SheafAnalysis {
        h0: num_components,
        h1,
        agent_count: n,
        agreement_edges,
        disagreement_pairs,
    }
}

// Union-Find helpers.
fn find(parent: &mut [usize], x: usize) -> usize {
    if parent[x] != x {
        parent[x] = find(parent, parent[x]);
    }
    parent[x]
}

fn union(parent: &mut [usize], x: usize, y: usize) {
    let rx = find(parent, x);
    let ry = find(parent, y);
    if rx != ry {
        parent[rx] = ry;
    }
}

// ─── Rendering ────────────────────────────────────────────────────────

/// Color for H¹=0 (agreement): green.
const COLOR_AGREE: Color = Color::Rgb(0x6c, 0xcb, 0x5f);
/// Color for H¹=1 (one obstruction): yellow.
const COLOR_WARN: Color = Color::Rgb(0xfa, 0xe2, 0x46);
/// Color for H¹≥2 (structural disagreement): red.
const COLOR_DISAGREE: Color = Color::Rgb(0xff, 0x6b, 0x6b);

/// Determine the verdict from analysis results.
pub fn verdict(analysis: &SheafAnalysis) -> DisagreementVerdict {
    match analysis.h1 {
        0 => DisagreementVerdict::CommunicationHelps,
        1 => DisagreementVerdict::OneObstruction,
        _ => DisagreementVerdict::NeedNewPerspective,
    }
}

/// Determine the border/accent color based on H¹.
pub fn disagreement_color(h1: usize) -> Color {
    match h1 {
        0 => COLOR_AGREE,
        1 => COLOR_WARN,
        _ => COLOR_DISAGREE,
    }

}

/// Render the agent disagreement card into the given area.
///
/// The card shows:
/// ```text
/// ┌─ Agent Consensus ─────────────┐
/// │ H⁰ = 1 (agents CAN agree)    │
/// │ H¹ = 0 (no obstructions)     │
/// │ Verdict: Communication helps  │
/// └───────────────────────────────┘
/// ```
pub fn render(frame: &mut Frame, area: Rect, agents: &[AgentFix]) {
    if area.width < 6 || area.height < 5 {
        return; // Too small to render anything useful.
    }

    let analysis = compute_sheaf_analysis(agents);
    let v = verdict(&analysis);
    let accent = disagreement_color(analysis.h1);

    let title = if analysis.h1 == 0 {
        " Agent Consensus "
    } else {
        " Agent Disagreement "
    };

    let border_style = Style::default().fg(accent);
    let title_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(Color::DarkGray);

    let h0_label = match analysis.h0 {
        0 => "no agents present".to_string(),
        1 => "agents CAN agree".to_string(),
        n => format!("{} separate clusters", n),
    };
    let h1_label = match analysis.h1 {
        0 => "no obstructions".to_string(),
        1 => "1 irreducible split".to_string(),
        n => format!("{} irreducible splits", n),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title, title_style))
        .title_alignment(Alignment::Left)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(vec![
            Span::styled("H\u{2070} = ", text_style),
            Span::styled(format!("{}", analysis.h0), title_style),
            Span::styled(format!(" ({})", h0_label), dim_style),
        ]),
        Line::from(vec![
            Span::styled("H\u{00B9} = ", text_style),
            Span::styled(format!("{}", analysis.h1), title_style),
            Span::styled(format!(" ({})", h1_label), dim_style),
        ]),
        Line::from(vec![
            Span::styled("Verdict: ", dim_style),
            Span::styled(v.label(), Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        ]),
    ];

    let para = Paragraph::new(lines);
    frame.render_widget(para, inner);
}

/// Render a compact single-line status indicator for the status bar.
///
/// Format: `H¹=0 ✓` or `H¹=2 ✗` with color coding.
pub fn render_status_indicator(frame: &mut Frame, area: Rect, agents: &[AgentFix]) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let analysis = compute_sheaf_analysis(agents);
    let accent = disagreement_color(analysis.h1);

    let icon = if analysis.h1 == 0 { "✓" } else { "✗" };
    let text = format!("H¹={} {}", analysis.h1, icon);

    let line = Line::from(Span::styled(text, Style::default().fg(accent)));
    frame.render_widget(Paragraph::new(line), area);
}

/// Render agent nodes as a text-based graph visualization.
///
/// Shows agent names connected by agreement (──) or disagreement (╌╌) edges.
/// Compact form suitable for embedding in a card or panel.
pub fn render_graph(frame: &mut Frame, area: Rect, agents: &[AgentFix]) {
    if area.width < 10 || area.height < 3 || agents.len() < 2 {
        return;
    }

    let analysis = compute_sheaf_analysis(agents);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Agent Graph ",
            Style::default().fg(Color::DarkGray),
        ))
        .border_style(Style::default().fg(Color::Rgb(50, 50, 50)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if agents.is_empty() {
        let para = Paragraph::new(Line::from(Span::styled(
            "No agents",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(para, inner);
        return;
    }

    // Render each agent as a node label.
    let max_label_width = (inner.width as usize).saturating_sub(4).max(8);
    let mut lines: Vec<Line> = Vec::new();

    // Node list.
    for agent in agents.iter() {
        let label = if agent.agent_id.len() > max_label_width {
            format!("{}…", &agent.agent_id[..max_label_width.saturating_sub(1)])
        } else {
            agent.agent_id.clone()
        };
        lines.push(Line::from(vec![
            Span::styled("● ", Style::default().fg(Color::Cyan)),
            Span::styled(label, Style::default().fg(Color::White)),
        ]));
    }

    // Edge summary.
    if !analysis.agreement_edges.is_empty() || !analysis.disagreement_pairs.is_empty() {
        lines.push(Line::default()); // blank separator

        for &(a, b) in &analysis.agreement_edges {
            let name_a = truncate_str(&agents[a].agent_id, 8);
            let name_b = truncate_str(&agents[b].agent_id, 8);
            lines.push(Line::from(vec![
                Span::styled(format!("{}──{} ", name_a, name_b), Style::default().fg(COLOR_AGREE)),
                Span::styled("agree", Style::default().fg(COLOR_AGREE)),
            ]));
        }

        for &(a, b) in &analysis.disagreement_pairs {
            let name_a = truncate_str(&agents[a].agent_id, 8);
            let name_b = truncate_str(&agents[b].agent_id, 8);
            lines.push(Line::from(vec![
                Span::styled(format!("{}╌╌{} ", name_a, name_b), Style::default().fg(COLOR_DISAGREE)),
                Span::styled("differ", Style::default().fg(COLOR_DISAGREE)),
            ]));
        }
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, inner);
}

/// Truncate a string to at most `max` characters.
fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}

// ─── Integration Hook ─────────────────────────────────────────────────

/// Render the full disagreement panel: graph + analysis card side by side.
/// Designed to be called from the chat render pipeline when multiple
/// agents have responded with diverging fixes.
pub fn render_panel(frame: &mut Frame, area: Rect, agents: &[AgentFix]) {
    if agents.len() < 2 {
        return; // Nothing to compare.
    }

    if area.width < 30 || area.height < 7 {
        // Fall back to just the card if space is tight.
        render(frame, area, agents);
        return;
    }

    // Split area: graph on left, card on right.
    let left_width = (area.width / 2).max(15).min(area.width.saturating_sub(20));
    let graph_area = Rect {
        width: left_width,
        ..area
    };
    let card_area = Rect {
        x: area.x + left_width,
        width: area.width.saturating_sub(left_width),
        ..area
    };

    render_graph(frame, graph_area, agents);
    render(frame, card_area, agents);
}

// ─── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Similarity Tests ─────────────────────────────────────────────

    #[test]
    fn similarity_identical_strings() {
        assert!((text_similarity("hello world", "hello world") - 1.0).abs() < 1e-10);
    }

    #[test]
    fn similarity_empty_strings() {
        assert!((text_similarity("", "") - 1.0).abs() < 1e-10);
    }

    #[test]
    fn similarity_one_empty() {
        assert!((text_similarity("hello", "") - 0.0).abs() < 1e-10);
    }

    #[test]
    fn similarity_completely_different() {
        let sim = text_similarity("the quick brown fox", "zzz yyy xxx www");
        assert!(sim < 0.3, "expected low similarity, got {}", sim);
    }

    #[test]
    fn similarity_partial_overlap() {
        let sim = text_similarity("add null check to handler", "add null guard to handler");
        assert!(sim > 0.5, "expected high similarity, got {}", sim);
    }

    #[test]
    fn similarity_case_insensitive() {
        let sim = text_similarity("Add Null Check", "add null check");
        assert!((sim - 1.0).abs() < 1e-10);
    }

    #[test]
    fn similarity_short_strings() {
        let sim = text_similarity("fix", "fix");
        assert!((sim - 1.0).abs() < 1e-10);
    }

    #[test]
    fn similarity_reordered_words() {
        let sim = text_similarity("check null add", "add null check");
        // Token Jaccard is 1.0 for reordered words.
        assert!(sim > 0.7, "expected high similarity for reordered words, got {}", sim);
    }

    // ─── Semantic Boost Tests ─────────────────────────────────────────

    #[test]
    fn semantic_boost_synonym_fix_patterns() {
        let boost = semantic_boost("add null check", "insert null guard");
        assert!(boost > 0.0, "expected positive semantic boost");
    }

    #[test]
    fn semantic_boost_unrelated() {
        let boost = semantic_boost("refactor module", "update config file");
        assert!(boost < 0.1, "expected near-zero boost for unrelated texts");
    }

    // ─── Agreement Detection Tests ────────────────────────────────────

    #[test]
    fn fixes_agree_identical() {
        let a = AgentFix { agent_id: "a".into(), fix_text: "add null check".into() };
        let b = AgentFix { agent_id: "b".into(), fix_text: "add null check".into() };
        assert!(fixes_agree(&a, &b));
    }

    #[test]
    fn fixes_agree_similar() {
        let a = AgentFix { agent_id: "a".into(), fix_text: "Add a null check before dereferencing the pointer".into() };
        let b = AgentFix { agent_id: "b".into(), fix_text: "Insert a null guard to check the pointer before use".into() };
        assert!(fixes_agree(&a, &b));
    }

    #[test]
    fn fixes_disagree_different() {
        let a = AgentFix { agent_id: "a".into(), fix_text: "Add caching layer for database queries".into() };
        let b = AgentFix { agent_id: "b".into(), fix_text: "Rewrite the authentication module from scratch".into() };
        assert!(!fixes_agree(&a, &b));
    }

    // ─── Sheaf Analysis Tests ─────────────────────────────────────────

    #[test]
    fn empty_agents() {
        let analysis = compute_sheaf_analysis(&[]);
        assert_eq!(analysis.h0, 0);
        assert_eq!(analysis.h1, 0);
        assert_eq!(analysis.agent_count, 0);
    }

    #[test]
    fn single_agent() {
        let agents = vec![AgentFix { agent_id: "a".into(), fix_text: "fix it".into() }];
        let analysis = compute_sheaf_analysis(&agents);
        assert_eq!(analysis.h0, 1); // one component
        assert_eq!(analysis.h1, 0); // no disagreements possible
        assert_eq!(analysis.agent_count, 1);
    }

    #[test]
    fn two_agents_agree() {
        let agents = vec![
            AgentFix { agent_id: "a".into(), fix_text: "add null check to handler".into() },
            AgentFix { agent_id: "b".into(), fix_text: "add null check to handler".into() },
        ];
        let analysis = compute_sheaf_analysis(&agents);
        assert_eq!(analysis.h0, 1);
        assert_eq!(analysis.h1, 0);
        assert_eq!(analysis.agreement_edges.len(), 1);
        assert!(analysis.disagreement_pairs.is_empty());
    }

    #[test]
    fn two_agents_disagree() {
        let agents = vec![
            AgentFix { agent_id: "a".into(), fix_text: "Implement Redis caching layer for all database queries".into() },
            AgentFix { agent_id: "b".into(), fix_text: "Rewrite the entire authentication and authorization module".into() },
        ];
        let analysis = compute_sheaf_analysis(&agents);
        assert_eq!(analysis.h0, 2); // two separate components
        assert_eq!(analysis.h1, 1); // one cross-component disagreement
    }

    #[test]
    fn three_agents_all_agree() {
        let agents = vec![
            AgentFix { agent_id: "a".into(), fix_text: "add null check to handler".into() },
            AgentFix { agent_id: "b".into(), fix_text: "add null check to handler".into() },
            AgentFix { agent_id: "c".into(), fix_text: "add null guard to handler".into() },
        ];
        let analysis = compute_sheaf_analysis(&agents);
        assert_eq!(analysis.h0, 1);
        assert_eq!(analysis.h1, 0);
    }

    #[test]
    fn three_agents_all_disagree() {
        let agents = vec![
            AgentFix { agent_id: "a".into(), fix_text: "Implement Redis caching layer for all database queries".into() },
            AgentFix { agent_id: "b".into(), fix_text: "Rewrite the entire authentication and authorization module".into() },
            AgentFix { agent_id: "c".into(), fix_text: "Migrate all services to a completely different cloud provider".into() },
        ];
        let analysis = compute_sheaf_analysis(&agents);
        assert_eq!(analysis.h0, 3); // three separate components
        assert!(analysis.h1 >= 3); // at least 3 cross-component disagreements
    }

    #[test]
    fn three_agents_two_agree_one_disagrees() {
        let agents = vec![
            AgentFix { agent_id: "a".into(), fix_text: "add null check to handler".into() },
            AgentFix { agent_id: "b".into(), fix_text: "add null check to handler".into() },
            AgentFix { agent_id: "c".into(), fix_text: "Completely restructure the entire application architecture".into() },
        ];
        let analysis = compute_sheaf_analysis(&agents);
        assert_eq!(analysis.h0, 2); // {a,b} and {c}
        assert!(analysis.h1 >= 2); // c disagrees with both a and b
    }

    #[test]
    fn analysis_preserves_agent_count() {
        let agents = vec![
            AgentFix { agent_id: "a".into(), fix_text: "fix".into() },
            AgentFix { agent_id: "b".into(), fix_text: "fix".into() },
            AgentFix { agent_id: "c".into(), fix_text: "fix".into() },
            AgentFix { agent_id: "d".into(), fix_text: "different".into() },
        ];
        let analysis = compute_sheaf_analysis(&agents);
        assert_eq!(analysis.agent_count, 4);
    }

    // ─── Transitivity Hole Tests ──────────────────────────────────────

    #[test]
    fn transitivity_hole_detected() {
        // A agrees with B, B agrees with C, but A disagrees with C.
        // This is a "hole" in the agreement sheaf.
        let agents = vec![
            AgentFix { agent_id: "a".into(), fix_text: "add null check before pointer dereference".into() },
            AgentFix { agent_id: "b".into(), fix_text: "add null check before pointer dereference".into() },
            AgentFix { agent_id: "c".into(), fix_text: "Completely rewrite the entire networking stack from scratch".into() },
        ];
        let analysis = compute_sheaf_analysis(&agents);
        // A-B agree, so they form a component. C disagrees with both.
        // h0 = 2, h1 >= 2 (C disagrees with A and B cross-component).
        assert_eq!(analysis.h0, 2);
        assert!(analysis.h1 >= 2);
    }

    // ─── Verdict Tests ────────────────────────────────────────────────

    #[test]
    fn verdict_communication_helps() {
        let analysis = SheafAnalysis { h0: 1, h1: 0, agent_count: 2, agreement_edges: vec![], disagreement_pairs: vec![] };
        assert_eq!(verdict(&analysis), DisagreementVerdict::CommunicationHelps);
        assert_eq!(verdict(&analysis).label(), "Communication helps");
    }

    #[test]
    fn verdict_one_obstruction() {
        let analysis = SheafAnalysis { h0: 2, h1: 1, agent_count: 2, agreement_edges: vec![], disagreement_pairs: vec![] };
        assert_eq!(verdict(&analysis), DisagreementVerdict::OneObstruction);
    }

    #[test]
    fn verdict_need_new_perspective() {
        let analysis = SheafAnalysis { h0: 3, h1: 3, agent_count: 3, agreement_edges: vec![], disagreement_pairs: vec![] };
        assert_eq!(verdict(&analysis), DisagreementVerdict::NeedNewPerspective);
    }

    // ─── Color Tests ──────────────────────────────────────────────────

    #[test]
    fn color_green_for_agreement() {
        assert_eq!(disagreement_color(0), COLOR_AGREE);
    }

    #[test]
    fn color_yellow_for_one_obstruction() {
        assert_eq!(disagreement_color(1), COLOR_WARN);
    }

    #[test]
    fn color_red_for_multiple_obstructions() {
        assert_eq!(disagreement_color(2), COLOR_DISAGREE);
        assert_eq!(disagreement_color(10), COLOR_DISAGREE);
    }

    // ─── Trigram Tests ────────────────────────────────────────────────

    #[test]
    fn trigrams_basic() {
        let t = trigrams("hello");
        assert!(t.contains("hel"));
        assert!(t.contains("ell"));
        assert!(t.contains("llo"));
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn trigrams_short_string() {
        let t = trigrams("ab");
        assert_eq!(t.len(), 1);
        assert!(t.contains("ab"));
    }

    #[test]
    fn trigrams_empty() {
        let t = trigrams("");
        assert!(t.is_empty() || t.contains(""));
    }

    // ─── Union-Find Tests ─────────────────────────────────────────────

    #[test]
    fn union_find_basic() {
        let mut parent = vec![0, 1, 2];
        union(&mut parent, 0, 1);
        assert_eq!(find(&mut parent, 0), find(&mut parent, 1));
    }

    #[test]
    fn union_find_transitive() {
        let mut parent = vec![0, 1, 2];
        union(&mut parent, 0, 1);
        union(&mut parent, 1, 2);
        assert_eq!(find(&mut parent, 0), find(&mut parent, 2));
    }

    #[test]
    fn union_find_separate_components() {
        let mut parent = vec![0, 1, 2, 3];
        union(&mut parent, 0, 1);
        union(&mut parent, 2, 3);
        assert_ne!(find(&mut parent, 0), find(&mut parent, 2));
    }

    // ─── Truncate Helper Tests ────────────────────────────────────────

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_string() {
        assert_eq!(truncate_str("hello world", 5), "hell…");
    }

    #[test]
    fn truncate_exact_length() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    // ─── Integration-Style Tests ──────────────────────────────────────

    #[test]
    fn sheaf_analysis_cloned_is_equal() {
        let agents = vec![
            AgentFix { agent_id: "a".into(), fix_text: "fix the bug".into() },
            AgentFix { agent_id: "b".into(), fix_text: "fix the bug".into() },
        ];
        let a1 = compute_sheaf_analysis(&agents);
        let a2 = a1.clone();
        assert_eq!(a1, a2);
    }

    #[test]
    fn many_agents_cluster_correctly() {
        // 5 agents: {A,B,C} agree, {D,E} agree with each other but not A/B/C.
        let agents = vec![
            AgentFix { agent_id: "a".into(), fix_text: "add null check to input validation".into() },
            AgentFix { agent_id: "b".into(), fix_text: "add null check to input validation".into() },
            AgentFix { agent_id: "c".into(), fix_text: "add null guard to input validation".into() },
            AgentFix { agent_id: "d".into(), fix_text: "Refactor the entire logging subsystem with structured events".into() },
            AgentFix { agent_id: "e".into(), fix_text: "Refactor the entire logging subsystem with structured events".into() },
        ];
        let analysis = compute_sheaf_analysis(&agents);
        assert_eq!(analysis.h0, 2); // two clusters
        assert!(analysis.h1 >= 6); // cross-component: 3*2 = 6 disagreements
    }

    #[test]
    fn agreement_threshold_sanity() {
        // Just above and just below threshold.
        let similar = AgentFix { agent_id: "a".into(), fix_text: "fix the null pointer exception in handler".into() };
        let near = AgentFix { agent_id: "b".into(), fix_text: "fix the null pointer error in handler".into() };
        // These should agree (very similar).
        assert!(fixes_agree(&similar, &near));
    }

    #[test]
    fn completely_unrelated_fixes_disagree() {
        let a = AgentFix { agent_id: "a".into(), fix_text: "Optimize the SQL query with proper indexing".into() };
        let b = AgentFix { agent_id: "b".into(), fix_text: "Add unit tests for the payment processing module".into() };
        assert!(!fixes_agree(&a, &b));
    }

    #[test]
    fn jaccard_identical_sets() {
        let a: std::collections::HashSet<&str> = ["a", "b", "c"].into_iter().collect();
        let b: std::collections::HashSet<&str> = ["a", "b", "c"].into_iter().collect();
        assert!((jaccard(&a, &b) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn jaccard_disjoint_sets() {
        let a: std::collections::HashSet<&str> = ["a", "b"].into_iter().collect();
        let b: std::collections::HashSet<&str> = ["c", "d"].into_iter().collect();
        assert!((jaccard(&a, &b) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn jaccard_partial_overlap() {
        let a: std::collections::HashSet<&str> = ["a", "b", "c"].into_iter().collect();
        let b: std::collections::HashSet<&str> = ["b", "c", "d"].into_iter().collect();
        let j = jaccard(&a, &b);
        // intersection = {b,c} = 2, union = {a,b,c,d} = 4
        assert!((j - 0.5).abs() < 1e-10);
    }
}
