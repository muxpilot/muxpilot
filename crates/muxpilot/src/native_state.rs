use crate::model::Selection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeGroup {
    Running,
    Configured,
    Directories,
    /// Agents-mode buckets (T#3): one row per agent-pane, grouped by how much it
    /// needs the user so the eye lands on the attention states first.
    AgentNeedsYou,
    AgentWorking,
    AgentQuiet,
}

impl NativeGroup {
    pub(crate) fn label(self) -> &'static str {
        let l = crate::ui::labels();
        match self {
            Self::Running => l.group_running,
            Self::Configured => l.group_configured,
            Self::Directories => l.group_dirs,
            Self::AgentNeedsYou => l.group_needs_you,
            Self::AgentWorking => l.group_working,
            Self::AgentQuiet => l.group_quiet,
        }
    }

    /// Render order (lower first). Agent buckets sort attention → working →
    /// quiet; they never mix with the workspace groups (each mode builds only
    /// its own kinds).
    pub(crate) fn order(self) -> u8 {
        match self {
            Self::Running => 0,
            Self::Configured => 1,
            Self::Directories => 2,
            Self::AgentNeedsYou => 3,
            Self::AgentWorking => 4,
            Self::AgentQuiet => 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NativeAction {
    Select(Selection),
}

/// One pane of a window, shown as a third-level tree leaf when a multi-pane
/// window is expanded. This is where a pane's real per-pane model/state lives,
/// so a multi-agent window never has to fake a single model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaneRow {
    /// tmux pane id (e.g. `%7`) — the switch target.
    pub(crate) id: String,
    /// Name column: the agent kind + model (e.g. `claude opus-4-8`) when this is
    /// an agent pane, else the running command.
    pub(crate) label: String,
    /// Status column: glyph + short state label for an agent pane, else empty.
    pub(crate) status: String,
    /// Whether this pane hosts a coding agent (drives spinner polling + color).
    pub(crate) agent: bool,
    /// Pre-rendered relative activity label (e.g. `3m`).
    pub(crate) activity: String,
}

/// One window of a running session, shown as an indented child row when the
/// session is expanded in the tree. Carries everything the picker needs to
/// render the row and to switch straight to that window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowRow {
    pub(crate) index: u32,
    /// tmux window id (e.g. `@4`) — the switch target.
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) active: bool,
    pub(crate) panes: usize,
    pub(crate) agents: usize,
    /// Pre-rendered relative activity label (e.g. `3m`), so the child row needs
    /// no clock at draw time and the demo can supply synthetic values.
    pub(crate) activity: String,
    /// Per-pane detail. Always populated (one entry per pane); the *adaptive*
    /// tree only opens a third level when there's more than one pane — a one-pane
    /// window shows its agent inline on the window row instead.
    pub(crate) pane_rows: Vec<PaneRow>,
}

impl WindowRow {
    /// Whether this window opens into a third level of pane leaves. Adaptive: a
    /// one-pane window stays a leaf (its single pane shows inline).
    pub(crate) fn is_expandable(&self) -> bool {
        self.pane_rows.len() > 1
    }

    /// The single agent pane's inline status, when the window holds exactly one
    /// agent pane — so a one-pane agent window surfaces its state on the row.
    pub(crate) fn inline_agent_status(&self) -> Option<&str> {
        match self.pane_rows.as_slice() {
            [only] if only.agent => Some(only.status.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NativeEntry {
    pub(crate) line: String,
    pub(crate) detail: String,
    pub(crate) search_text: String,
    pub(crate) action: NativeAction,
    pub(crate) tags: Vec<&'static str>,
    pub(crate) group: NativeGroup,
    /// Session name — set only for running sessions, so window children can be
    /// switched to via `Selection::Window`.
    pub(crate) session: Option<String>,
    /// Child windows, shown when the row is expanded. Empty for non-sessions.
    pub(crate) windows: Vec<WindowRow>,
    /// When true, the name column holds a filesystem path — render it with a
    /// middle ellipsis (keep the `~/` head and the file tail) instead of the
    /// default trailing clip, so the important ends stay visible.
    pub(crate) name_is_path: bool,
}

impl NativeEntry {
    pub(crate) fn new(
        line: String,
        detail: String,
        action: NativeAction,
        tags: Vec<&'static str>,
        group: NativeGroup,
    ) -> Self {
        let search_text = format!("{line}\n{detail}\n{}", tags.join(" ")).to_ascii_lowercase();
        Self {
            line,
            detail,
            search_text,
            action,
            tags,
            group,
            session: None,
            windows: Vec::new(),
            name_is_path: false,
        }
    }

    /// Attach the session's windows so the row can expand into a tree.
    pub(crate) fn with_windows(mut self, session: String, windows: Vec<WindowRow>) -> Self {
        self.session = Some(session);
        self.windows = windows;
        self
    }

    /// Mark the name column as a filesystem path (middle-ellipsis on overflow).
    pub(crate) fn with_name_as_path(mut self) -> Self {
        self.name_is_path = true;
        self
    }

    /// Whether this row has windows to expand.
    pub(crate) fn is_expandable(&self) -> bool {
        !self.windows.is_empty()
    }
}

/// The picker's top-level mode. Each mode is a distinct list built by its own
/// function and picks the *honest* unit for its question: Sessions is the
/// running-session tree, Agents is one row per agent-pane (so model/state are
/// never ambiguous), Layouts is the startable tmuxinator inventory, Dirs is the
/// directory picker. Switched via footer command keys `s`/`a`/`x`/`d` (and
/// cycled with Tab). The visible list is then narrowed by a plain substring
/// filter ([`crate::ui::entry_matches`]) — there is no per-mode scope filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PickerMode {
    Sessions,
    Agents,
    Layouts,
    Dirs,
}

impl PickerMode {
    pub(crate) fn label(self) -> &'static str {
        let l = crate::ui::labels();
        match self {
            Self::Sessions => l.mode_sessions,
            Self::Agents => l.mode_agents,
            Self::Layouts => l.mode_layouts,
            Self::Dirs => l.mode_dirs,
        }
    }

    /// The switch key as a string, for footer key hints.
    pub(crate) fn key_label(self) -> &'static str {
        match self {
            Self::Sessions => "s",
            Self::Agents => "a",
            Self::Layouts => "l",
            Self::Dirs => "d",
        }
    }

    /// Cycle order for Tab.
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Sessions => Self::Agents,
            Self::Agents => Self::Layouts,
            Self::Layouts => Self::Dirs,
            Self::Dirs => Self::Sessions,
        }
    }

    /// All modes in switch order, for the footer switcher.
    pub(crate) const ALL: [Self; 4] = [Self::Sessions, Self::Agents, Self::Layouts, Self::Dirs];

    /// Stable identifier used to persist the last-used tab across runs.
    pub(crate) fn as_key(self) -> &'static str {
        match self {
            Self::Sessions => "sessions",
            Self::Agents => "agents",
            Self::Layouts => "layouts",
            Self::Dirs => "dirs",
        }
    }

    /// Parse a persisted key back into a mode (unknown/blank → None).
    pub(crate) fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_key() == key.trim())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FilterInput {
    text: String,
    cursor: usize,
}

impl FilterInput {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    pub(crate) fn insert(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    pub(crate) fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = self.prev_boundary(self.cursor);
        self.text.replace_range(prev..self.cursor, "");
        self.cursor = prev;
    }

    pub(crate) fn delete_word_before_cursor(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut start = self.cursor;
        while start > 0 {
            let prev = self.prev_boundary(start);
            let ch = self.text[prev..start].chars().next().unwrap_or_default();
            if !ch.is_whitespace() {
                break;
            }
            start = prev;
        }
        while start > 0 {
            let prev = self.prev_boundary(start);
            let ch = self.text[prev..start].chars().next().unwrap_or_default();
            if ch.is_whitespace() {
                break;
            }
            start = prev;
        }
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    pub(crate) fn move_left(&mut self) {
        self.cursor = self.prev_boundary(self.cursor);
    }

    pub(crate) fn move_right(&mut self) {
        self.cursor = self.next_boundary(self.cursor);
    }

    pub(crate) fn move_start(&mut self) {
        self.cursor = 0;
    }

    pub(crate) fn move_end(&mut self) {
        self.cursor = self.text.len();
    }

    pub(crate) fn display_with_cursor(&self) -> String {
        let mut out = String::with_capacity(self.text.len() + "█".len());
        out.push_str(&self.text[..self.cursor]);
        out.push('█');
        out.push_str(&self.text[self.cursor..]);
        out
    }

    fn prev_boundary(&self, cursor: usize) -> usize {
        self.text[..cursor]
            .char_indices()
            .last()
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }

    fn next_boundary(&self, cursor: usize) -> usize {
        if cursor >= self.text.len() {
            return self.text.len();
        }
        self.text[cursor..]
            .char_indices()
            .nth(1)
            .map(|(idx, _)| cursor + idx)
            .unwrap_or(self.text.len())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Theme {
    /// Status bar background + dim text.
    pub(crate) title: &'static str,
    /// Accent brand mark on the status bar.
    pub(crate) brand: &'static str,
    /// Body background + normal text.
    pub(crate) panel: &'static str,
    /// Faint uppercase section labels in the detail pane.
    pub(crate) panel_header: &'static str,
    /// Accent title inside the detail pane.
    pub(crate) detail_title: &'static str,
    /// Group section header label (dim, bold).
    pub(crate) group: &'static str,
    /// Hairline rules and the preview divider.
    pub(crate) divider: &'static str,
    /// Selected row background + text.
    pub(crate) selected: &'static str,
    /// Left accent marker on the selected row.
    pub(crate) marker: &'static str,
    /// Footer background + dim text.
    pub(crate) footer: &'static str,
    /// Accent keycaps in the footer.
    pub(crate) key: &'static str,
    /// Active filter prompt.
    pub(crate) filter_active: &'static str,
    /// Highlighted filter match.
    pub(crate) match_highlight: &'static str,
    /// Running (non-agent) state glyph.
    pub(crate) active: &'static str,
    /// Configured/idle state glyph.
    pub(crate) ready: &'static str,
    /// Agent-present state glyph.
    pub(crate) agent: &'static str,
    /// Current-workspace state glyph.
    pub(crate) current: &'static str,
}

/// The active picker theme. Runtime-toggled between [`THEME_DARK`] and
/// [`THEME_LIGHT`]; the default honours `$MUXPILOT_THEME` (`light`/`dark`).
pub(crate) fn default_theme() -> &'static Theme {
    match std::env::var("MUXPILOT_THEME").ok().as_deref() {
        Some("light") => &THEME_LIGHT,
        _ => &THEME_DARK,
    }
}

impl Theme {
    /// The other theme — used by the runtime `t` toggle.
    pub(crate) fn toggled(&self) -> &'static Theme {
        if std::ptr::eq(self, &THEME_LIGHT) {
            &THEME_DARK
        } else {
            &THEME_LIGHT
        }
    }
}

// Terminal-tasker palette, dark: warm near-black ground, terracotta accent.
pub(crate) const THEME_DARK: Theme = Theme {
    title: "\x1b[48;5;236m\x1b[38;5;245m",
    brand: "\x1b[48;5;236m\x1b[38;5;209m\x1b[1m",
    panel: "\x1b[48;5;234m\x1b[38;5;253m",
    panel_header: "\x1b[48;5;234m\x1b[38;5;240m\x1b[1m",
    detail_title: "\x1b[48;5;234m\x1b[38;5;209m\x1b[1m",
    group: "\x1b[48;5;234m\x1b[38;5;245m\x1b[1m",
    divider: "\x1b[48;5;234m\x1b[38;5;240m",
    selected: "\x1b[48;5;130m\x1b[38;5;230m\x1b[1m",
    marker: "\x1b[48;5;130m\x1b[38;5;223m\x1b[1m",
    footer: "\x1b[48;5;236m\x1b[38;5;245m",
    key: "\x1b[48;5;236m\x1b[38;5;209m\x1b[1m",
    filter_active: "\x1b[48;5;236m\x1b[38;5;209m\x1b[1m",
    match_highlight: "\x1b[48;5;209m\x1b[38;5;234m\x1b[1m",
    active: "\x1b[48;5;234m\x1b[38;5;78m",
    ready: "\x1b[48;5;234m\x1b[38;5;240m",
    agent: "\x1b[48;5;234m\x1b[38;5;214m",
    current: "\x1b[48;5;234m\x1b[38;5;209m",
};

// Terminal-tasker palette, light: warm near-white ground, same terracotta family.
pub(crate) const THEME_LIGHT: Theme = Theme {
    title: "\x1b[48;5;253m\x1b[38;5;240m",
    brand: "\x1b[48;5;253m\x1b[38;5;166m\x1b[1m",
    panel: "\x1b[48;5;255m\x1b[38;5;235m",
    panel_header: "\x1b[48;5;255m\x1b[38;5;246m\x1b[1m",
    detail_title: "\x1b[48;5;255m\x1b[38;5;166m\x1b[1m",
    group: "\x1b[48;5;255m\x1b[38;5;240m\x1b[1m",
    divider: "\x1b[48;5;255m\x1b[38;5;246m",
    selected: "\x1b[48;5;223m\x1b[38;5;235m\x1b[1m",
    marker: "\x1b[48;5;223m\x1b[38;5;166m\x1b[1m",
    footer: "\x1b[48;5;253m\x1b[38;5;240m",
    key: "\x1b[48;5;253m\x1b[38;5;166m\x1b[1m",
    filter_active: "\x1b[48;5;253m\x1b[38;5;166m\x1b[1m",
    match_highlight: "\x1b[48;5;216m\x1b[38;5;235m\x1b[1m",
    active: "\x1b[48;5;255m\x1b[38;5;71m",
    ready: "\x1b[48;5;255m\x1b[38;5;246m",
    agent: "\x1b[48;5;255m\x1b[38;5;136m",
    current: "\x1b[48;5;255m\x1b[38;5;166m",
};

pub(crate) const KEY_BINDINGS: &[(&str, &str, &str)] = &[
    ("Enter", "open", "switch/start selected workspace or window"),
    ("j/k or Up/Down", "move", "move selected row"),
    ("1-9 then ↑/↓", "jump", "jump that many rows up/down (relative-number gutter)"),
    ("t or →", "tree", "expand the tree one level: session → window → pane"),
    ("Space", "tree", "toggle the current tree node"),
    ("←", "close", "collapse the current tree level"),
    ("g/G", "edge", "jump to first/last row"),
    ("Ctrl-D/Ctrl-U", "page", "move half page down/up"),
    ("/", "filter", "edit filter"),
    ("Esc", "normal", "clear count, leave filter/help, or close menu"),
    ("s/a/l/d", "mode", "switch to sessions/agents/layouts/dirs"),
    ("Tab", "mode", "cycle to the next mode"),
    ("?", "help", "toggle this help"),
    ("T", "theme", "toggle light/dark theme"),
    ("r", "refresh", "refresh tmux state"),
    ("q or Ctrl-C", "close", "close menu"),
    ("Backspace", "filter", "delete previous filter character"),
    ("Ctrl-W", "filter", "delete previous filter word"),
    ("Ctrl-A/Ctrl-E", "filter", "move to start/end of filter"),
    ("Ctrl-B/Ctrl-F", "filter", "move filter cursor left/right"),
    ("Left/Right", "filter", "move filter cursor left/right"),
];

pub(crate) fn native_help_body() -> Vec<String> {
    let mut lines = vec![
        "tmux menu help".to_string(),
        String::new(),
        "Keys".to_string(),
    ];
    for (key, label, description) in KEY_BINDINGS {
        lines.push(format!("  {key:<17} {label:<8} {description}"));
    }
    lines.extend([
        String::new(),
        "Modes".to_string(),
        "  s sessions  running tmux sessions, expandable into their window tree".to_string(),
        "  a agents    one row per agent-pane; model + state are never ambiguous".to_string(),
        "  l layouts   tmuxinator inventory, flagged running/stopped".to_string(),
        "  d dirs      directory picker for configured or bare sessions".to_string(),
        String::new(),
        "Rows".to_string(),
        "  ◆ current workspace, ● running, ○ configured/directory".to_string(),
        "  Space/t expands session → window → pane; a 1-pane window shows inline".to_string(),
        "   tmux session,  windows,  panes, 󰚩 agents".to_string(),
        "  󰐊 tmuxinator layout/project,  directory,  local config".to_string(),
        "  RUNNING active tmux sessions".to_string(),
        "  TMUXINATOR repo-local layouts, saved projects, configured repos".to_string(),
    ]);
    // Agent-state legend — generated from the enum so glyphs/colours never drift
    // from what the picker actually draws. Documents both shape and hue.
    lines.push(String::new());
    lines.push("Agent states".to_string());
    for status in crate::snapshot::PaneAgentStatus::ALL {
        lines.push(format!(
            "  {} {:<15} {}",
            status.glyph(),
            status.as_str(),
            status.description()
        ));
    }
    lines.push(
        "  attention states (approve/input/error/rate) bubble to the session row".to_string(),
    );
    lines.extend([
        String::new(),
        "CLI".to_string(),
        "  muxpilot state [--json] [--capture]".to_string(),
        "  muxpilot switch <session>".to_string(),
        "  muxpilot window <window-id>".to_string(),
        "  muxpilot pane <pane-id>".to_string(),
        "  muxpilot --fzf".to_string(),
    ]);
    lines
}
