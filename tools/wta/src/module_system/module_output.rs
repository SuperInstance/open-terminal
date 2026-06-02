//! Output types that modules can produce.
//!
//! Modules are guests — they suggest, observe, and analyze.
//! They cannot commandeer the UI or block the event loop.
//! All output is advisory and presented at the terminal's discretion.

/// A single output item from a module.
#[derive(Debug, Clone, PartialEq)]
pub enum ModuleOutput {
    /// Compact text for the status bar (e.g. "λ₂=0.34 h=0.21").
    StatusBar(String),
    /// Non-blocking toast notification.
    Notification(String),
    /// Inline suggestion in the prompt area (e.g. "try: cargo test").
    InlineHint(String),
    /// Mini bar chart visualization.
    BarChart {
        /// Chart label.
        label: String,
        /// Data values.
        values: Vec<f64>,
    },
    /// Longer-form observation for the agent/insight pane.
    Insight(String),
}

impl ModuleOutput {
    /// Create a status bar output.
    pub fn status_bar(text: impl Into<String>) -> Self {
        ModuleOutput::StatusBar(text.into())
    }

    /// Create a notification output.
    pub fn notification(text: impl Into<String>) -> Self {
        ModuleOutput::Notification(text.into())
    }

    /// Create an inline hint.
    pub fn inline_hint(text: impl Into<String>) -> Self {
        ModuleOutput::InlineHint(text.into())
    }

    /// Create an insight.
    pub fn insight(text: impl Into<String>) -> Self {
        ModuleOutput::Insight(text.into())
    }

    /// Create a bar chart.
    pub fn bar_chart(label: impl Into<String>, values: Vec<f64>) -> Self {
        ModuleOutput::BarChart {
            label: label.into(),
            values,
        }
    }

    /// A human-readable tag for the output kind.
    pub fn kind_tag(&self) -> &'static str {
        match self {
            ModuleOutput::StatusBar(_) => "status",
            ModuleOutput::Notification(_) => "notification",
            ModuleOutput::InlineHint(_) => "hint",
            ModuleOutput::BarChart { .. } => "chart",
            ModuleOutput::Insight(_) => "insight",
        }
    }

    /// Extract text content if this is a text-based output.
    pub fn text_content(&self) -> Option<&str> {
        match self {
            ModuleOutput::StatusBar(s) => Some(s),
            ModuleOutput::Notification(s) => Some(s),
            ModuleOutput::InlineHint(s) => Some(s),
            ModuleOutput::Insight(s) => Some(s),
            ModuleOutput::BarChart { .. } => None,
        }
    }

    /// Estimated "weight" for prioritization (higher = more prominent).
    pub fn weight(&self) -> u8 {
        match self {
            ModuleOutput::StatusBar(_) => 1,
            ModuleOutput::InlineHint(_) => 2,
            ModuleOutput::Notification(_) => 3,
            ModuleOutput::BarChart { .. } => 4,
            ModuleOutput::Insight(_) => 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_bar_output() {
        let out = ModuleOutput::status_bar("λ₂=0.34");
        assert_eq!(out.kind_tag(), "status");
        assert_eq!(out.text_content(), Some("λ₂=0.34"));
        assert_eq!(out.weight(), 1);
    }

    #[test]
    fn notification_output() {
        let out = ModuleOutput::notification("entropy high");
        assert_eq!(out.kind_tag(), "notification");
        assert_eq!(out.text_content(), Some("entropy high"));
        assert_eq!(out.weight(), 3);
    }

    #[test]
    fn inline_hint_output() {
        let out = ModuleOutput::inline_hint("try: cargo test");
        assert_eq!(out.kind_tag(), "hint");
        assert_eq!(out.text_content(), Some("try: cargo test"));
    }

    #[test]
    fn bar_chart_output() {
        let out = ModuleOutput::bar_chart("strengths", vec![0.5, 0.8, 0.3]);
        assert_eq!(out.kind_tag(), "chart");
        assert!(out.text_content().is_none());
        assert_eq!(out.weight(), 4);
    }

    #[test]
    fn insight_output() {
        let out = ModuleOutput::insight("You always run build then test");
        assert_eq!(out.kind_tag(), "insight");
        assert_eq!(out.weight(), 5);
    }

    #[test]
    fn equality() {
        assert_eq!(
            ModuleOutput::StatusBar("x".into()),
            ModuleOutput::StatusBar("x".into())
        );
        assert_ne!(
            ModuleOutput::StatusBar("x".into()),
            ModuleOutput::StatusBar("y".into())
        );
        assert_eq!(
            ModuleOutput::BarChart {
                label: "a".into(),
                values: vec![1.0]
            },
            ModuleOutput::BarChart {
                label: "a".into(),
                values: vec![1.0]
            }
        );
    }

    #[test]
    fn text_content_variants() {
        assert!(ModuleOutput::status_bar("s").text_content().is_some());
        assert!(ModuleOutput::notification("n").text_content().is_some());
        assert!(ModuleOutput::inline_hint("h").text_content().is_some());
        assert!(ModuleOutput::insight("i").text_content().is_some());
        assert!(ModuleOutput::bar_chart("c", vec![]).text_content().is_none());
    }

    #[test]
    fn weight_ordering() {
        assert!(ModuleOutput::status_bar("").weight() < ModuleOutput::inline_hint("").weight());
        assert!(ModuleOutput::inline_hint("").weight() < ModuleOutput::notification("").weight());
        assert!(ModuleOutput::notification("").weight() < ModuleOutput::bar_chart("", vec![]).weight());
        assert!(ModuleOutput::bar_chart("", vec![]).weight() < ModuleOutput::insight("").weight());
    }
}
