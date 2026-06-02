//! # Spectral Analysis of Agent Collaboration Networks
//!
//! Given active agent sessions (e.g. Copilot, Claude, Codex, Gemini),
//! builds a collaboration graph where nodes are agents and edges
//! represent shared context (same project, overlapping file edits,
//! cross-referenced conversations).
//!
//! ## Metrics
//!
//! - **Fiedler value** (algebraic connectivity): Second-smallest Laplacian
//!   eigenvalue. High values indicate a well-connected network.
//! - **Cheeger constant**: The isoperimetric number of the graph. Small
//!   values indicate a bottleneck — a cut that restricts information flow.
//! - **Mixing time**: How many communication steps needed for information
//!   from one agent to spread across the network.
//!
//! ## Status Bar Display
//!
//! A compact indicator for the TUI status bar:
//!
//! ```text
//! λ₂=0.34 h=0.21
//! ```

use nalgebra::DMatrix;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A node (agent) in the collaboration graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNode {
    /// Agent identifier (e.g. "copilot", "claude").
    pub id: String,
    /// Display label.
    pub label: String,
    /// Whether the session is currently alive.
    pub alive: bool,
}

/// A weighted edge between two agent nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollabEdge {
    /// Source node index.
    pub source: usize,
    /// Target node index.
    pub target: usize,
    /// Edge weight (shared context strength, 0.0–1.0).
    pub weight: f64,
}

/// The collaboration graph for spectral analysis.
#[derive(Debug, Clone)]
pub struct AgentGraph {
    /// Nodes (agents).
    pub nodes: Vec<AgentNode>,
    /// Weighted edges.
    pub edges: Vec<CollabEdge>,
    /// Adjacency matrix (computed from edges).
    adjacency: Option<DMatrix<f64>>,
    /// Laplacian matrix (computed from adjacency).
    laplacian: Option<DMatrix<f64>>,
    /// Cached spectral metrics.
    cached_fiedler: Option<f64>,
    cached_cheeger: Option<f64>,
    cached_mixing_time: Option<usize>,
    /// Whether the cache is dirty.
    dirty: bool,
}

impl AgentGraph {
    /// Create a new empty agent collaboration graph.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            adjacency: None,
            laplacian: None,
            cached_fiedler: None,
            cached_cheeger: None,
            cached_mixing_time: None,
            dirty: true,
        }
    }

    /// Add a node to the graph.
    pub fn add_node(&mut self, id: &str, label: &str, alive: bool) {
        self.nodes.push(AgentNode {
            id: id.to_string(),
            label: label.to_string(),
            alive,
        });
        self.invalidate_cache();
    }

    /// Add a weighted edge between two agents by their id strings.
    /// Returns an error if either id is unknown.
    pub fn add_edge(&mut self, source_id: &str, target_id: &str, weight: f64) -> Result<(), String> {
        let source = self
            .nodes
            .iter()
            .position(|n| n.id == source_id)
            .ok_or_else(|| format!("unknown source agent: {source_id}"))?;
        let target = self
            .nodes
            .iter()
            .position(|n| n.id == target_id)
            .ok_or_else(|| format!("unknown target agent: {target_id}"))?;

        // Avoid duplicate edges; update weight if exists.
        for e in &mut self.edges {
            if (e.source == source && e.target == target)
                || (e.source == target && e.target == source)
            {
                e.weight = weight.max(e.weight);
                self.dirty = true;
                return Ok(());
            }
        }

        self.edges.push(CollabEdge {
            source,
            target,
            weight: weight.clamp(0.0, 1.0),
        });
        self.dirty = true;
        Ok(())
    }

    /// Remove an agent node by id.
    pub fn remove_node(&mut self, id: &str) {
        if let Some(pos) = self.nodes.iter().position(|n| n.id == id) {
            self.nodes.remove(pos);
            // Remove edges that reference this node.
            self.edges.retain(|e| e.source != pos && e.target != pos);
            // Re-index edges for nodes after the removed one.
            for e in &mut self.edges {
                if e.source > pos {
                    e.source -= 1;
                }
                if e.target > pos {
                    e.target -= 1;
                }
            }
            self.dirty = true;
        }
    }

    /// Number of nodes in the graph.
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Number of edges in the graph.
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// Rebuild the adjacency and Laplacian matrices from edges.
    fn rebuild_matrices(&mut self) {
        if !self.dirty {
            return;
        }

        let n = self.nodes.len();
        if n == 0 {
            self.adjacency = None;
            self.laplacian = None;
            return;
        }

        let mut adj = DMatrix::zeros(n, n);
        for e in &self.edges {
            adj[(e.source, e.target)] = e.weight;
            adj[(e.target, e.source)] = e.weight; // symmetric
        }
        self.adjacency = Some(adj.clone());

        // Laplacian L = D - A.
        let mut lap = DMatrix::zeros(n, n);
        for i in 0..n {
            let deg: f64 = adj.row(i).iter().sum();
            lap[(i, i)] = deg;
            for j in 0..n {
                if i != j {
                    lap[(i, j)] = -adj[(i, j)];
                }
            }
        }
        self.laplacian = Some(lap);
        self.dirty = false;
    }

    /// Get a reference to the adjacency matrix. Rebuilds if dirty.
    pub fn adjacency_matrix(&mut self) -> Option<&DMatrix<f64>> {
        self.rebuild_matrices();
        self.adjacency.as_ref()
    }

    /// Get a reference to the Laplacian matrix. Rebuilds if dirty.
    pub fn laplacian_matrix(&mut self) -> Option<&DMatrix<f64>> {
        self.rebuild_matrices();
        self.laplacian.as_ref()
    }

    /// Compute the Fiedler value (algebraic connectivity): the second-smallest
    /// eigenvalue of the Laplacian.
    ///
    /// Uses the Rayleigh quotient iteration to approximate λ₂. Returns `None`
    /// for graphs with fewer than 2 nodes.
    pub fn fiedler_value(&mut self) -> Option<f64> {
        if let Some(cached) = self.cached_fiedler {
            if !self.dirty {
                return Some(cached);
            }
        }

        self.rebuild_matrices();
        let n = self.nodes.len();
        if n < 2 {
            return None;
        }

        // Compute the two smallest eigenvalues via power iteration on
        // the shifted inverse (Rayleigh quotient iteration).
        let lap = self.laplacian.as_ref().unwrap();
        let eigenvalues = Self::compute_two_smallest_eigenvalues(lap, 2000, 1e-10);
        eprintln!("@fiedler_value n={} lap[:3]: {:?} evals:{:?}", n, lap.iter().take(9).copied().collect::<Vec<_>>(), eigenvalues);
        if eigenvalues.len() < 2 {
            return None;
        }

        // λ₂ is the second eigenvalue (index 1).
        let fiedler = eigenvalues[1];
        self.cached_fiedler = Some(fiedler);
        Some(fiedler)
    }

    /// Compute the Cheeger constant from the Fiedler eigenvector sweep.
    ///
    /// The Cheeger constant h(G) = min_{S} |∂S| / min(vol(S), vol(S̅)) where
    /// |∂S| is the size of the boundary of S. Returns `None` for graphs
    /// with fewer than 2 nodes.
    pub fn cheeger_constant(&mut self) -> Option<f64> {
        if let Some(cached) = self.cached_cheeger {
            if !self.dirty {
                return Some(cached);
            }
        }

        self.rebuild_matrices();
        let n = self.nodes.len();
        if n < 2 {
            return None;
        }

        let lap = self.laplacian.as_ref().unwrap();
        let eigenvalues = Self::compute_two_smallest_eigenvalues(lap, 2000, 1e-10);
        if eigenvalues.len() < 2 {
            return None;
        }

        // Get the eigenvector for λ₂.
        let eigenvector = Self::rayleigh_quotient_eigenvector(lap, &eigenvalues, 2000, 1e-10);
        let fiedler_vec = match eigenvector.get(&(1usize)) {
            Some(v) => v.clone(),
            None => {
                // Placeholder: uniform vector if we couldn't converge.
                nalgebra::DVector::from_element(n, 1.0)
            }
        };

        // Sweep cut: sort vertex indices by Fiedler vector entry, consider
        // each prefix as a candidate cut set S.
        let mut indices: Vec<usize> = (0..n).collect();
        indices.sort_by(|&a, &b| {
            fiedler_vec[a]
                .partial_cmp(&fiedler_vec[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Total volume.
        let adj = self.adjacency.as_ref().unwrap();
        let vol_total: f64 = adj.row_iter().map(|r| r.iter().sum::<f64>()).sum();

        let mut best_h = f64::MAX;
        let mut vol_s = 0.0;

        for k in 1..n {
            let cut_idx = indices[k - 1];
            vol_s += adj.row(cut_idx).iter().sum::<f64>();

            // Boundary: edges from S to S̅.
            let boundary = self.compute_boundary(&indices[..k], &indices[k..], adj);
            let vol_sbar = vol_total - vol_s;

            if vol_s > 0.0 && vol_sbar > 0.0 {
                let h = boundary / vol_s.min(vol_sbar);
                if h < best_h {
                    best_h = h;
                }
            }
        }

        self.cached_cheeger = Some(best_h);
        Some(best_h)
    }

    /// Compute the boundary size between two partitions.
    fn compute_boundary(
        &self,
        s_indices: &[usize],
        sbar_indices: &[usize],
        adj: &DMatrix<f64>,
    ) -> f64 {
        let mut boundary = 0.0;
        for &i in s_indices {
            for &j in sbar_indices {
                boundary += adj[(i, j)];
            }
        }
        boundary
    }

    /// Estimate the mixing time: the number of steps for a random walk
    /// on the graph to approach the stationary distribution.
    ///
    /// Uses the approximation τ ≈ 1 / λ₂ (for well-connected graphs).
    pub fn mixing_time(&mut self) -> Option<usize> {
        if let Some(cached) = self.cached_mixing_time {
            if !self.dirty {
                return Some(cached);
            }
        }

        let n = self.nodes.len();
        if n < 2 {
            return None;
        }

        let fiedler = self.fiedler_value()?;
        if fiedler <= 0.0 || fiedler >= 2.0 {
            // Degenerate or disconnected.
            return Some(usize::MAX);
        }

        // Mixing time ~ log(1/ε) / (λ₂) for a lazy random walk.
        // Use ε = 0.01.
        let epsilon: f64 = 0.01;
        let mt = (-epsilon.ln() / fiedler).ceil() as usize;
        self.cached_mixing_time = Some(mt);
        Some(mt)
    }

    /// Compute the two smallest eigenvalues of a symmetric matrix using
    /// the power method with deflation.
    fn compute_two_smallest_eigenvalues(matrix: &DMatrix<f64>, max_iter: usize, tol: f64) -> Vec<f64> {
        let n = matrix.nrows();
        if n == 0 {
            return vec![];
        }
        if n == 1 {
            return vec![matrix[(0, 0)]];
        }

        // Shift the matrix by a large positive constant so the smallest
        // eigenvalues become the largest shifted ones for power iteration.
        // Find approximate spectral radius for shifting.
        let shift = matrix
            .iter()
            .map(|v| v.abs())
            .fold(0.0_f64, f64::max)
            + 1.0;

        // Use the shifted matrix: B = shift * I - A.
        // The eigenvalues of B are (shift - λ_i). The largest eigenvalue
        // of B corresponds to the smallest λ_i of A.

        let shifted = DMatrix::from_fn(n, n, |i, j| {
            if i == j {
                shift - matrix[(i, j)]
            } else {
                -matrix[(i, j)]
            }
        });

        // Find the largest eigenvalue of the shifted matrix (gives λ_min).
        let lambda_max_shifted =
            Self::power_iteration_max_eigenvalue(&shifted, max_iter, tol);
        let lambda_min = shift - lambda_max_shifted;

        // Deflate: subtract the component of the largest eigenvector of the
        // shifted matrix, then find the next largest.
        let v1 = Self::power_iteration_eigenvector(&shifted, max_iter, tol);
        let deflated = &shifted
            - (lambda_max_shifted
                * &v1
                * v1.transpose());

        let lambda_second_shifted =
            Self::power_iteration_max_eigenvalue(&deflated, max_iter, tol);
        let lambda_second = shift - lambda_second_shifted;

        // λ₁ ≤ λ₂ ≤ ... ≤ λ_n. Sort ascending.
        let mut result = vec![lambda_min, lambda_second];
        result.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        result
    }

    /// Power iteration to find the largest eigenvalue of a symmetric matrix.
    fn power_iteration_max_eigenvalue(matrix: &DMatrix<f64>, max_iter: usize, tol: f64) -> f64 {
        let n = matrix.nrows();
        if n == 0 {
            return 0.0;
        }
        let mut v = nalgebra::DVector::from_element(n, 1.0 / (n as f64).sqrt());
        let mut lambda_old = 0.0;

        for _ in 0..max_iter {
            let w = matrix * &v;
            let lambda = v.dot(&w);
            let norm = w.norm();
            if norm <= 1e-15 {
                return 0.0;
            }
            v = w / norm;

            if (lambda - lambda_old).abs() < tol {
                return lambda;
            }
            lambda_old = lambda;
        }

        // Rayleigh quotient at the final iteration.
        let w = matrix * &v;
        v.dot(&w)
    }

    /// Power iteration to find the eigenvector for the largest eigenvalue.
    fn power_iteration_eigenvector(
        matrix: &DMatrix<f64>,
        max_iter: usize,
        tol: f64,
    ) -> nalgebra::DVector<f64> {
        let n = matrix.nrows();
        if n == 0 {
            return nalgebra::DVector::zeros(0);
        }
        let mut v = nalgebra::DVector::from_element(n, 1.0 / (n as f64).sqrt());

        for _ in 0..max_iter {
            let w = matrix * &v;
            let w_old = v.clone();
            let norm = w.norm();
            if norm <= 1e-15 {
                return v;
            }
            v = w / norm;

            if (&v - &w_old).norm() < tol {
                break;
            }
        }
        v
    }

    /// Rayleigh quotient iteration: compute the eigenvector for a specific
    /// eigenvalue, for all computed eigenvalues.
    fn rayleigh_quotient_eigenvector(
        matrix: &DMatrix<f64>,
        eigenvalues: &[f64],
        max_iter: usize,
        tol: f64,
    ) -> HashMap<usize, nalgebra::DVector<f64>> {
        let n = matrix.nrows();
        let mut map = std::collections::HashMap::new();

        for (idx, &target_lambda) in eigenvalues.iter().enumerate() {
            let mut v = nalgebra::DVector::from_element(n, 1.0 / (n as f64).sqrt());
            // Inverse iteration: (A - μI)^{-1} v.
            let shifted = matrix.clone() - DMatrix::identity(n, n).scale(target_lambda);

            for _ in 0..max_iter {
                // Solve (A - μI) w = v via LU decomposition.
                let lu = shifted.clone().lu();
                let solved = lu.solve(&v);
                let w = match solved {
                    Some(s) => s,
                    None => break, // singular, give up
                };

                let norm = w.norm();
                if norm <= 1e-15 {
                    break;
                }
                let w_next = w / norm;

                if (&w_next - &v).norm() < tol {
                    v = w_next;
                    break;
                }
                v = w_next;
            }

            map.insert(idx, v);
        }

        map
    }

    /// Compute a compact status bar display string.
    ///
    /// Format: `λ₂=0.34 h=0.21 t=3` or empty string for small graphs.
    pub fn status_bar_indicator(&mut self) -> String {
        let n = self.nodes.len();
        if n < 2 {
            return String::new();
        }

        let fiedler = self.fiedler_value().unwrap_or(0.0);
        let cheeger = self.cheeger_constant().unwrap_or(0.0);
        let mixing = self.mixing_time().unwrap_or(0);

        format!("λ₂={fiedler:.2} h={cheeger:.2} t={mixing}")
    }

    /// Reset all cached spectral values so they are recomputed.
    pub fn invalidate_cache(&mut self) {
        self.dirty = true;
        self.cached_fiedler = None;
        self.cached_cheeger = None;
        self.cached_mixing_time = None;
    }
}

impl Default for AgentGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// A dashboard that holds an agent graph and provides periodic recomputation.
#[derive(Debug, Clone)]
pub struct SpectralDashboard {
    /// The agent collaboration graph.
    pub graph: AgentGraph,
    /// Last computed Fiedler value.
    pub last_fiedler: Option<f64>,
    /// Last computed Cheeger constant.
    pub last_cheeger: Option<f64>,
    /// Last computed mixing time.
    pub last_mixing_time: Option<usize>,
    /// Tick counter for periodic recomputation.
    ticks_since_update: u64,
    /// Recompute interval in ticks (default 10).
    recompute_interval: u64,
}

impl SpectralDashboard {
    /// Create a new spectral dashboard with the default recompute interval.
    pub fn new() -> Self {
        Self {
            graph: AgentGraph::new(),
            last_fiedler: None,
            last_cheeger: None,
            last_mixing_time: None,
            ticks_since_update: 0,
            recompute_interval: 10,
        }
    }

    /// Set the recompute interval (in ticks).
    pub fn set_recompute_interval(&mut self, interval: u64) {
        self.recompute_interval = interval;
    }

    /// Called on each app tick. Periodically recomputes spectral metrics.
    pub fn tick(&mut self) {
        self.ticks_since_update += 1;
        if self.ticks_since_update >= self.recompute_interval {
            self.recompute();
            self.ticks_since_update = 0;
        }
    }

    /// Force recomputation of spectral metrics.
    pub fn recompute(&mut self) {
        if self.graph.num_nodes() >= 2 {
            self.last_fiedler = self.graph.fiedler_value();
            self.last_cheeger = self.graph.cheeger_constant();
            self.last_mixing_time = self.graph.mixing_time();
        }
    }

    /// Get the status bar indicator string.
    pub fn status_bar_indicator(&mut self) -> String {
        self.graph.status_bar_indicator()
    }
}

impl Default for SpectralDashboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Graph Construction Tests ─────────────────────────────────────

    #[test]
    fn empty_graph() {
        let g = AgentGraph::new();
        assert_eq!(g.num_nodes(), 0);
        assert_eq!(g.num_edges(), 0);
    }

    #[test]
    fn add_nodes() {
        let mut g = AgentGraph::new();
        g.add_node("copilot", "Copilot", true);
        g.add_node("claude", "Claude", true);
        assert_eq!(g.num_nodes(), 2);
    }

    #[test]
    fn add_edge_connects_nodes() {
        let mut g = AgentGraph::new();
        g.add_node("a", "Agent A", true);
        g.add_node("b", "Agent B", true);
        assert!(g.add_edge("a", "b", 0.8).is_ok());
        assert_eq!(g.num_edges(), 1);
    }

    #[test]
    fn add_edge_unknown_node_fails() {
        let mut g = AgentGraph::new();
        assert!(g.add_edge("nonexistent", "b", 0.5).is_err());
    }

    #[test]
    fn remove_node_removes_edges() {
        let mut g = AgentGraph::new();
        g.add_node("a", "A", true);
        g.add_node("b", "B", true);
        g.add_node("c", "C", true);
        let _ = g.add_edge("a", "b", 1.0);
        let _ = g.add_edge("b", "c", 1.0);
        g.remove_node("b");
        assert_eq!(g.num_nodes(), 2);
        assert_eq!(g.num_edges(), 0);
    }

    #[test]
    fn duplicate_edge_updates_weight() {
        let mut g = AgentGraph::new();
        g.add_node("x", "X", true);
        g.add_node("y", "Y", true);
        let _ = g.add_edge("x", "y", 0.5);
        let _ = g.add_edge("y", "x", 0.9);
        assert_eq!(g.num_edges(), 1);
        // Weight should be max(0.5, 0.9) = 0.9
        assert!((g.edges[0].weight - 0.9).abs() < 1e-10);
    }

    #[test]
    fn adjacency_matrix_symmetric() {
        let mut g = AgentGraph::new();
        g.add_node("a", "A", true);
        g.add_node("b", "B", true);
        let _ = g.add_edge("a", "b", 0.7);

        let adj = g.adjacency_matrix().unwrap();
        assert!((adj[(0, 1)] - 0.7).abs() < 1e-10);
        assert!((adj[(1, 0)] - 0.7).abs() < 1e-10);
    }

    // ─── Spectral Tests ───────────────────────────────────────────────

    #[test]
    fn fiedler_value_two_nodes_full_edge() {
        let mut g = AgentGraph::new();
        g.add_node("a", "A", true);
        g.add_node("b", "B", true);
        let _ = g.add_edge("a", "b", 1.0);

        let f = g.fiedler_value();
        assert!(f.is_some());
        let fv = f.unwrap();
        // For a 2-node graph connected by weight 1.0:
        // Laplacian = [[1, -1], [-1, 1]], eigenvalues = [0, 2].
        // Fiedler value = 2.
        assert!((fv - 2.0).abs() < 0.1, "Fiedler value should be ~2.0, got {fv}");
    }

    #[test]
    fn fiedler_value_disconnected_graph() {
        let mut g = AgentGraph::new();
        g.add_node("a", "A", true);
        g.add_node("b", "B", true);
        g.add_node("c", "C", true);
        // Only connect a-b, leave c isolated.
        let _ = g.add_edge("a", "b", 1.0);

        let f = g.fiedler_value();
        assert!(f.is_some());
        let fv = f.unwrap();
        // Disconnected component means λ₂ = 0.
        assert!(
            fv < 0.1,
            "disconnected graph should have near-zero Fiedler value, got {fv}"
        );
    }

    #[test]
    fn cheeger_constant_connected_pair() {
        let mut g = AgentGraph::new();
        g.add_node("a", "A", true);
        g.add_node("b", "B", true);
        let _ = g.add_edge("a", "b", 1.0);

        let h = g.cheeger_constant();
        assert!(h.is_some());
        let hv = h.unwrap();
        // For 2-node graph with single edge: Cheeger = 1 / 1 = 1 (volumes are 1 each).
        // Actually: boundary = 1.0, min(vol(S), vol(S̅)) = min(1, 1) = 1
        // h = 1. The exact value might differ slightly due to numerical sweep.
        assert!(hv > 0.5 && hv <= 1.5, "Cheeger should be ~1.0, got {hv}");
    }

    #[test]
    fn mixing_time_finite() {
        let mut g = AgentGraph::new();
        g.add_node("a", "A", true);
        g.add_node("b", "B", true);
        g.add_node("c", "C", true);
        g.add_node("d", "D", true);
        // Complete graph with weak weights.
        let _ = g.add_edge("a", "b", 0.5);
        let _ = g.add_edge("b", "c", 0.5);
        let _ = g.add_edge("c", "d", 0.5);
        let _ = g.add_edge("d", "a", 0.5);
        let _ = g.add_edge("a", "c", 0.3);
        let _ = g.add_edge("b", "d", 0.3);

        let mt = g.mixing_time();
        assert!(mt.is_some());
        let mtv = mt.unwrap();
        assert!(
            mtv < 1000,
            "mixing time should be reasonable: got {mtv}"
        );
    }

    #[test]
    fn fiedler_none_for_single_node() {
        let mut g = AgentGraph::new();
        g.add_node("only", "Only", true);
        assert!(g.fiedler_value().is_none());
    }

    #[test]
    fn cheeger_none_for_single_node() {
        let mut g = AgentGraph::new();
        g.add_node("only", "Only", true);
        assert!(g.cheeger_constant().is_none());
    }

    #[test]
    fn status_bar_indicator_empty_for_small_graph() {
        let mut g = AgentGraph::new();
        g.add_node("a", "A", true);
        assert!(g.status_bar_indicator().is_empty());
    }

    #[test]
    fn status_bar_indicator_format() {
        let mut g = AgentGraph::new();
        g.add_node("a", "A", true);
        g.add_node("b", "B", true);
        let _ = g.add_edge("a", "b", 1.0);

        let indicator = g.status_bar_indicator();
        assert!(!indicator.is_empty());
        assert!(indicator.contains("λ₂="));
        assert!(indicator.contains("h="));
        assert!(indicator.contains("t="));
    }

    // ─── Dashboard Tests ──────────────────────────────────────────────

    #[test]
    fn dashboard_empty_on_creation() {
        let db = SpectralDashboard::new();
        assert!(db.last_fiedler.is_none());
        assert!(db.last_cheeger.is_none());
        assert_eq!(db.graph.num_nodes(), 0);
    }

    #[test]
    fn dashboard_recompute_two_nodes() {
        let mut db = SpectralDashboard::new();
        db.graph.add_node("cli", "CLI Agent", true);
        db.graph.add_node("code", "Code Agent", true);
        let _ = db.graph.add_edge("cli", "code", 0.8);
        db.recompute();
        assert!(db.last_fiedler.is_some());
        assert!(db.last_cheeger.is_some());
        assert!(db.last_mixing_time.is_some());
    }

    #[test]
    fn dashboard_tick_calls_recompute() {
        let mut db = make_dashboard_with_interval(2);
        db.graph.add_node("a", "A", true);
        db.graph.add_node("b", "B", true);
        let _ = db.graph.add_edge("a", "b", 1.0);
        // After 2 ticks, recompute should have been called.
        db.tick();
        assert!(db.last_fiedler.is_none()); // not yet
        db.tick();
        assert!(db.last_fiedler.is_some()); // after 2nd tick
    }

    #[test]
    fn dashboard_set_recompute_interval() {
        let mut db = SpectralDashboard::new();
        db.set_recompute_interval(5);
        assert_eq!(db.recompute_interval, 5);
    }

    // ─── Cache Invalidation ───────────────────────────────────────────

    #[test]
    fn invalidation_clears_cached_values() {
        let mut g = AgentGraph::new();
        g.add_node("x", "X", true);
        g.add_node("y", "Y", true);
        let _ = g.add_edge("x", "y", 1.0);
        let _ = g.fiedler_value();
        g.invalidate_cache();
        assert!(g.cached_fiedler.is_none());
        assert!(g.cached_cheeger.is_none());
        assert!(g.cached_mixing_time.is_none());
    }

    #[test]
    fn adding_node_invalidates_cache() {
        let mut g = AgentGraph::new();
        g.add_node("a", "A", true);
        g.add_node("b", "B", true);
        let _ = g.add_edge("a", "b", 1.0);
        let _ = g.fiedler_value();
        assert!(g.cached_fiedler.is_some());
        g.add_node("c", "C", true);
        assert!(g.cached_fiedler.is_none());
    }

    fn make_dashboard_with_interval(interval: u64) -> SpectralDashboard {
        let mut db = SpectralDashboard::new();
        db.recompute_interval = interval;
        db
    }
}
