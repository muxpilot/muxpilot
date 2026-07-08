//! Centralized, localization-ready UI text and glyphs.
//!
//! Every user-facing string the picker draws lives in a [`Labels`] table, and
//! [`labels()`] selects one by `$MUXPILOT_LANG` — mirroring how the theme is
//! selected by `$MUXPILOT_THEME`. Only English (`EN`) ships today, but adding a
//! locale is now purely additive: define another `Labels` const and route it in
//! `labels()`. No draw code hardcodes a string.
//!
//! Glyphs are universal (not translated), so they live in a single [`GLYPHS`]
//! const — the one place a marker/connector/icon character is written, so
//! "use a named constant, not a magic literal" holds across the UI.

/// All translatable chrome the picker renders.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Labels {
    pub(crate) brand: &'static str,
    /// Detail/preview pane header.
    pub(crate) details: &'static str,
    /// Help overlay title.
    pub(crate) help: &'static str,
    /// Filter footer prompt (uppercase in the footer).
    pub(crate) filter_prompt: &'static str,

    // Picker modes (also the status-line name).
    pub(crate) mode_sessions: &'static str,
    pub(crate) mode_agents: &'static str,
    pub(crate) mode_layouts: &'static str,
    pub(crate) mode_dirs: &'static str,

    // Group section headers.
    pub(crate) group_running: &'static str,
    pub(crate) group_configured: &'static str,
    pub(crate) group_dirs: &'static str,
    pub(crate) group_needs_you: &'static str,
    pub(crate) group_working: &'static str,
    pub(crate) group_quiet: &'static str,

    // Footer action labels.
    pub(crate) action_open: &'static str,
    pub(crate) action_start: &'static str,
    pub(crate) action_tree: &'static str,
    pub(crate) action_filter: &'static str,
    pub(crate) action_help: &'static str,
    pub(crate) action_close: &'static str,

    // Row status words.
    pub(crate) status_running: &'static str,
    pub(crate) status_stopped: &'static str,
    pub(crate) status_active: &'static str,
    pub(crate) status_tmuxinator: &'static str,

    // Layout-mode kind tags.
    pub(crate) kind_layout: &'static str,
    pub(crate) kind_project: &'static str,

    // Directory-mode words.
    pub(crate) dir_configured: &'static str,
    pub(crate) dir_bare: &'static str,
}

pub(crate) const EN: Labels = Labels {
    brand: "muxpilot",
    details: "details",
    help: "help",
    filter_prompt: "FILTER",

    mode_sessions: "sessions",
    mode_agents: "agents",
    mode_layouts: "layouts",
    mode_dirs: "dirs",

    group_running: "RUNNING",
    group_configured: "CONFIGURED",
    group_dirs: "DIRS",
    group_needs_you: "NEEDS YOU",
    group_working: "WORKING",
    group_quiet: "QUIET",

    action_open: "open",
    action_start: "start",
    action_tree: "tree",
    action_filter: "filter",
    action_help: "help",
    action_close: "close",

    status_running: "running",
    status_stopped: "stopped",
    status_active: "active",
    status_tmuxinator: "tmuxinator",

    kind_layout: "layout",
    kind_project: "project",

    dir_configured: "configured",
    dir_bare: "bare",
};

/// The active label set, selected by `$MUXPILOT_LANG` (English by default).
// The single arm is the locale-selection seam: add `Some("de") => &DE,` etc.
// above the wildcard as locales land. Kept as a `match` so that stays a one-line
// change rather than a control-flow rewrite.
#[allow(clippy::match_single_binding)]
pub(crate) fn labels() -> &'static Labels {
    match std::env::var("MUXPILOT_LANG").ok().as_deref() {
        _ => &EN,
    }
}

/// Universal glyphs used across the picker — one named home each, so no draw
/// site carries a bare geometric literal. Not localized (glyphs are language
/// independent); agent-state glyphs live on `PaneAgentStatus::glyph`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Glyphs {
    /// Current-workspace marker.
    pub(crate) current: &'static str,
    /// Running-session marker.
    pub(crate) running: &'static str,
    /// Configured/idle/directory marker.
    pub(crate) idle: &'static str,
    /// Agent-count icon in the caps column.
    pub(crate) agents: &'static str,
    /// Selected-row accent bar.
    pub(crate) marker: &'static str,
    /// Preview divider.
    pub(crate) divider: &'static str,
    /// Filter echo icon in the status bar.
    pub(crate) filter: &'static str,
    /// Tree connector for a non-last child.
    pub(crate) tree_mid: &'static str,
    /// Tree connector for the last child.
    pub(crate) tree_last: &'static str,
}

pub(crate) const GLYPHS: Glyphs = Glyphs {
    current: "◆",
    running: "●",
    idle: "○",
    agents: "◍",
    marker: "▍",
    divider: "│",
    filter: "⌕",
    tree_mid: "├─",
    tree_last: "└─",
};
