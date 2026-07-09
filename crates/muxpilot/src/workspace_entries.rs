use std::collections::{BTreeMap, HashMap};
use std::time::SystemTime;

use crate::model::{DirItem, Layout, MenuModel, Selection};
use crate::native_state::{NativeAction, NativeEntry, NativeGroup};
use crate::snapshot::{AgentState, PaneAgentStatus, TmuxPane, TmuxSnapshot, TmuxWindow};
use crate::ui::{entry_sort_name, labels, spinner_frame, GLYPHS};

#[derive(Debug, Clone, Default)]
pub(crate) struct WorkspaceRow {
    pub(crate) name: String,
    pub(crate) path: Option<String>,
    pub(crate) display_path: Option<String>,
    pub(crate) session: Option<String>,
    pub(crate) layout: Option<Layout>,
    pub(crate) project: Option<String>,
    pub(crate) dir: Option<DirItem>,
    pub(crate) current: bool,
    pub(crate) windows: usize,
    pub(crate) panes: usize,
    pub(crate) agents: Vec<String>,
    pub(crate) window_details: Vec<WindowSummary>,
    pub(crate) agent_attention: bool,
    /// Any agent in this workspace changed its screen since the last snapshot
    /// (T3) — drives the honest "working" vs "idle" status label.
    pub(crate) agent_active: bool,
    /// The most-severe agent status among this workspace's panes (T4), so a
    /// session row surfaces the state that most needs the user.
    pub(crate) agent_status: Option<PaneAgentStatus>,
    pub(crate) last_activity: Option<u64>,
}

/// Fleet-wide counts of agent panes by coarse state, shown in the status bar.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FleetSummary {
    pub(crate) working: usize,
    pub(crate) waiting: usize,
    pub(crate) idle: usize,
}

impl FleetSummary {
    pub(crate) fn is_empty(self) -> bool {
        self.working == 0 && self.waiting == 0 && self.idle == 0
    }
}

/// Count agent panes across the whole snapshot by coarse state: waiting (needs
/// you) takes precedence, then actively working, else idle.
pub(crate) fn fleet_summary(snapshot: &TmuxSnapshot) -> FleetSummary {
    let mut s = FleetSummary::default();
    for session in &snapshot.sessions {
        for window in &session.windows {
            for pane in &window.panes {
                if let Some(agent) = &pane.agent {
                    if agent.attention {
                        s.waiting += 1;
                    } else if agent.status == PaneAgentStatus::Working || agent.is_active {
                        s.working += 1;
                    } else if agent.status == PaneAgentStatus::Idle {
                        s.idle += 1;
                    }
                    // Unknown/Parked are not asserted as idle — they weren't observed.
                }
            }
        }
    }
    s
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowSummary {
    pub(crate) index: u32,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) active: bool,
    pub(crate) panes: usize,
    pub(crate) agents: usize,
    pub(crate) last_activity: Option<u64>,
    /// Per-pane detail for the third tree level (one entry per pane).
    pub(crate) pane_rows: Vec<crate::native_state::PaneRow>,
}

/// Build a third-level pane leaf: an agent pane shows its kind + model and state
/// glyph; a plain pane shows its running command.
fn pane_row(pane: &crate::snapshot::TmuxPane) -> crate::native_state::PaneRow {
    let (label, status, agent) = match &pane.agent {
        Some(a) => {
            let label = match a.model.as_deref() {
                Some(m) => format!("{} {m}", a.kind),
                None => a.kind.clone(),
            };
            let status = format!("{} {}", a.status.glyph(), a.status.short_label());
            (label, status, true)
        }
        None => {
            let cmd = if pane.current_command.is_empty() {
                "shell".to_string()
            } else {
                pane.current_command.clone()
            };
            (cmd, String::new(), false)
        }
    };
    crate::native_state::PaneRow {
        id: pane.id.clone(),
        label,
        status,
        agent,
        activity: relative_activity(
            pane.last_activity
                .or_else(|| pane.agent.as_ref().and_then(|a| a.last_change)),
        ),
    }
}

impl WorkspaceRow {
    fn group(&self) -> NativeGroup {
        if self.session.is_some() {
            NativeGroup::Running
        } else {
            NativeGroup::Configured
        }
    }

    fn selection(&self) -> Selection {
        if let Some(session) = &self.session {
            return Selection::Session(session.clone());
        }
        if let Some(layout) = &self.layout {
            return Selection::Layout {
                session: layout.session.clone(),
                full_path: layout.path.clone(),
            };
        }
        if let Some(project) = &self.project {
            return Selection::Project(project.clone());
        }
        if let Some(dir) = &self.dir {
            return Selection::Dir {
                full_path: dir.path.clone(),
                has_local_config: dir.has_local_config,
            };
        }
        Selection::Session(self.name.clone())
    }

    fn dir_has_config(&self) -> bool {
        self.dir.as_ref().is_some_and(|dir| dir.has_local_config) || self.layout.is_some()
    }

    fn tags(&self) -> Vec<&'static str> {
        let mut tags = Vec::new();
        if self.session.is_some() {
            tags.push("session");
            tags.push("window");
        }
        if !self.agents.is_empty() {
            tags.push("agent");
        }
        if self.layout.is_some() {
            tags.push("layout");
            tags.push("project");
        }
        if self.project.is_some() {
            tags.push("project");
        }
        if self.dir.is_some() {
            tags.push("dir");
        }
        tags
    }
}

fn workspace_key_for_path(path: &str) -> String {
    format!("path:{path}")
}

fn workspace_key_for_session(session: &str) -> String {
    format!("session:{session}")
}

fn merge_workspace_by_session<'a>(
    rows: &'a mut BTreeMap<String, WorkspaceRow>,
    session: &str,
) -> &'a mut WorkspaceRow {
    rows.entry(workspace_key_for_session(session))
        .or_insert_with(|| WorkspaceRow {
            name: session.to_string(),
            session: Some(session.to_string()),
            ..Default::default()
        })
}

fn merge_workspace_by_path<'a>(
    rows: &'a mut BTreeMap<String, WorkspaceRow>,
    path: &str,
    fallback_name: &str,
) -> &'a mut WorkspaceRow {
    rows.entry(workspace_key_for_path(path))
        .or_insert_with(|| WorkspaceRow {
            name: fallback_name.to_string(),
            path: Some(path.to_string()),
            ..Default::default()
        })
}

/// Compact, fixed-width-safe capability tokens for the list's `caps` column.
///
/// Uses only single-cell glyphs (digits, `w`, `◍`) so column math via
/// `ui::columns` matches what the terminal actually renders — no nerd-font PUA
/// glyphs whose display width is unreliable. The `◍` keeps a trailing space
/// before its count: the geometric glyph paints slightly wider than its one
/// reported cell, so an adjacent digit would overlap it. Richer detail lives in
/// the preview.
fn capability_icons(row: &WorkspaceRow) -> String {
    let mut icons = Vec::new();
    if row.windows > 0 {
        icons.push(format!("{}w", row.windows));
    }
    if !row.agents.is_empty() {
        icons.push(format!("{} {}", GLYPHS.agents, row.agents.len()));
    }
    icons.join(" ")
}

/// Map the captured per-window summaries into displayable child rows.
fn window_rows(row: &WorkspaceRow) -> Vec<crate::native_state::WindowRow> {
    row.window_details
        .iter()
        .map(|w| crate::native_state::WindowRow {
            index: w.index,
            id: w.id.clone(),
            name: w.name.clone(),
            active: w.active,
            panes: w.panes,
            agents: w.agents,
            activity: relative_activity(w.last_activity),
            pane_rows: w.pane_rows.clone(),
        })
        .collect()
}

fn relative_activity(timestamp: Option<u64>) -> String {
    let Some(timestamp) = timestamp else {
        return "-".to_string();
    };
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(timestamp);
    let elapsed = now.saturating_sub(timestamp);
    match elapsed {
        0..=59 => "now".to_string(),
        60..=3599 => format!("{}m", elapsed / 60),
        3600..=86_399 => format!("{}h", elapsed / 3600),
        _ => format!("{}d", elapsed / 86_400),
    }
}

fn workspace_activity(row: &WorkspaceRow) -> String {
    if !row.agents.is_empty() {
        // Attention states win — the row says *what* needs the user (T4 severity
        // bubble). agent_attention can also be set by a hook flag on a
        // non-attention status, hence the fallback.
        if row.agent_attention {
            if let Some(status) = row.agent_status.filter(|s| s.needs_attention()) {
                return format!("{} {}", status.glyph(), status.short_label());
            }
            return format!("{} attn", PaneAgentStatus::WaitingInput.glyph());
        }
        // Working when the status says so (hook or screen classification) OR the
        // screen is actively changing (T3). Keying off status too is what makes
        // a hook-reported `working` — and the first picker open — render right.
        if row.agent_status == Some(PaneAgentStatus::Working) || row.agent_active {
            return format!("{} work", spinner_frame());
        }
        // Otherwise show the bubbled state honestly — an idle prompt reads
        // `idle`, a process-only detection reads `?`, not a blanket idle.
        let status = row.agent_status.unwrap_or(PaneAgentStatus::Unknown);
        return format!("{} {}", status.glyph(), status.short_label());
    }
    if row.session.is_some() {
        return labels().status_active.to_string();
    }
    // The group header says TMUXINATOR — name the specific kind here instead of
    // echoing it, so the status column tells the user which tmuxinator config
    // this is (a repo-local layout vs a saved global project).
    if row.layout.is_some() {
        return labels().kind_layout.to_string();
    }
    if row.project.is_some() {
        return labels().kind_project.to_string();
    }
    String::new()
}

pub(crate) fn workspace_detail(row: &WorkspaceRow) -> String {
    let mut lines = vec![
        "Workspace".to_string(),
        format!("Name: {}", row.name),
        format!("State: {}", row.group().label().to_ascii_lowercase()),
        format!("Capabilities: {}", capability_detail(row)),
        format!("Last activity: {}", relative_activity(row.last_activity)),
    ];
    if let Some(session) = &row.session {
        lines.push(format!("Session: {session}"));
        lines.push(format!("Windows: {}", row.windows));
        lines.push(format!("Panes: {}", row.panes));
        if !row.window_details.is_empty() {
            lines.push("Windows".to_string());
            for window in &row.window_details {
                let active = if window.active { "*" } else { " " };
                let agent = if window.agents == 0 {
                    String::new()
                } else {
                    format!(" 󰚩{}", window.agents)
                };
                lines.push(format!(
                    "  {active} {}:{} {}  {}{}  {}",
                    window.index,
                    window.id,
                    window.name,
                    window.panes,
                    agent,
                    relative_activity(window.last_activity)
                ));
            }
        }
    }
    // Show a location wherever we have one — prefer the pretty `~/…` display
    // path, falling back to the absolute path. Covers directories, running
    // sessions with a cwd, and repo-local layouts merged with their directory.
    if let Some(path) = row.display_path.as_deref().or(row.path.as_deref()) {
        lines.push(format!("Path: {path}"));
    }
    if let Some(layout) = &row.layout {
        lines.push(format!("Layout: {} ({})", layout.session, layout.path));
    }
    if let Some(project) = &row.project {
        lines.push(format!("Project: {project}"));
    }
    if !row.agents.is_empty() {
        lines.push(format!("Agents: {}", row.agents.join(", ")));
    }
    lines.join("\n")
}

fn capability_detail(row: &WorkspaceRow) -> String {
    let mut parts = Vec::new();
    if row.session.is_some() {
        parts.push(" session".to_string());
    }
    if row.windows > 0 {
        parts.push(format!(" {} windows", row.windows));
    }
    if row.panes > 0 {
        parts.push(format!(" {} panes", row.panes));
    }
    if !row.agents.is_empty() {
        parts.push(format!("󰚩 {} agents", row.agents.len()));
    }
    if row.layout.is_some() || row.project.is_some() {
        parts.push("󰐊 layout/project".to_string());
    }
    if row.dir.is_some() {
        parts.push(" directory".to_string());
    }
    if row.dir_has_config() {
        parts.push(" config".to_string());
    }
    parts.join(", ")
}

fn workspace_line(row: &WorkspaceRow) -> String {
    let state = if row.current {
        GLYPHS.current
    } else if row.session.is_some() {
        GLYPHS.running
    } else {
        GLYPHS.idle
    };
    format!(
        "{state} {} · {} · {} · {}",
        row.name,
        capability_icons(row),
        workspace_activity(row),
        relative_activity(row.last_activity)
    )
}

/// Collect the merged workspace rows (sessions + startable layouts/projects +
/// configured dirs), keyed and deduped. The per-mode builders filter and map
/// this into their own entry lists.
fn collect_workspace_rows(model: &MenuModel, snapshot: &TmuxSnapshot) -> Vec<WorkspaceRow> {
    let mut rows: BTreeMap<String, WorkspaceRow> = BTreeMap::new();
    let mut path_to_session: HashMap<String, String> = HashMap::new();

    for session in &model.sessions {
        let row = merge_workspace_by_session(&mut rows, &session.name);
        row.session = Some(session.name.clone());
        row.current = session.name == model.current;
        row.windows = row.windows.max(session.windows as usize);
    }

    for layout in &model.layouts {
        let key_session = workspace_key_for_session(&layout.session);
        let row = if rows.contains_key(&key_session) || layout.running {
            merge_workspace_by_session(&mut rows, &layout.session)
        } else {
            merge_workspace_by_path(&mut rows, &layout.path, &layout.session)
        };
        row.name = layout.session.clone();
        row.path = Some(layout.path.clone());
        row.display_path = Some(layout.display.clone());
        row.layout = Some(layout.clone());
        if layout.running {
            row.session = Some(layout.session.clone());
        }
        path_to_session.insert(layout.path.clone(), layout.session.clone());
    }

    for project in &model.projects {
        let row = rows
            .entry(format!("project:{project}"))
            .or_insert_with(|| WorkspaceRow {
                name: project.clone(),
                project: Some(project.clone()),
                ..Default::default()
            });
        row.project = Some(project.clone());
    }

    for dir in model.zoxide.iter().chain(model.plain_repos.iter()) {
        if !dir.has_local_config {
            continue;
        }
        let path_key = workspace_key_for_path(&dir.path);
        let key = if rows.contains_key(&path_key) {
            path_key
        } else {
            path_to_session
                .get(&dir.path)
                .map(|session| workspace_key_for_session(session))
                .unwrap_or_else(|| workspace_key_for_path(&dir.path))
        };
        let fallback_name = dir
            .display
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(&dir.display);
        let row = rows.entry(key).or_insert_with(|| WorkspaceRow {
            name: fallback_name.to_string(),
            path: Some(dir.path.clone()),
            ..Default::default()
        });
        row.path = Some(dir.path.clone());
        row.display_path = Some(dir.display.clone());
        row.dir = Some(dir.clone());
    }

    for session in &snapshot.sessions {
        let row = merge_workspace_by_session(&mut rows, &session.name);
        row.session = Some(session.name.clone());
        row.current = session.name == snapshot.current_session;
        row.windows = row.windows.max(session.windows.len());
        for window in &session.windows {
            row.last_activity = row.last_activity.max(window.last_activity);
            row.panes += window.panes.len();
            let agent_count = window
                .panes
                .iter()
                .filter(|pane| pane.agent.is_some())
                .count();
            row.window_details.push(WindowSummary {
                index: window.index,
                id: window.id.clone(),
                name: window.name.clone(),
                active: window.active,
                panes: window.panes.len(),
                agents: agent_count,
                last_activity: window.last_activity,
                pane_rows: window.panes.iter().map(pane_row).collect(),
            });
            for pane in &window.panes {
                row.last_activity = row.last_activity.max(pane.last_activity);
                if row.path.is_none() && !pane.path.is_empty() {
                    row.path = Some(pane.path.clone());
                }
                if let Some(agent) = &pane.agent {
                    let model = agent
                        .model
                        .as_deref()
                        .map(|m| format!(" {m}"))
                        .unwrap_or_default();
                    row.agents.push(format!(
                        "{}:{}{} {}% pane {}",
                        agent.kind, agent.status, model, agent.confidence, pane.id
                    ));
                    row.agent_attention |= agent.attention;
                    row.agent_active |= agent.is_active;
                    // Bubble the most-severe child state up to the row (T4).
                    row.agent_status = Some(match row.agent_status {
                        Some(cur) if cur.severity() >= agent.status.severity() => cur,
                        _ => agent.status,
                    });
                    // Prefer the content-change time over tmux pane_activity, which
                    // a repainting spinner keeps falsely fresh.
                    row.last_activity = row.last_activity.max(agent.last_change);
                }
            }
        }
    }

    rows.into_values()
        .filter(|row| {
            row.session.is_some()
                || row.layout.is_some()
                || row.project.is_some()
                || row.dir.is_some()
        })
        .collect()
}

/// Turn workspace rows into sorted picker entries (group order, then name).
/// Running sessions carry their windows so the row can expand into the tree.
fn rows_to_entries(rows: Vec<WorkspaceRow>) -> Vec<NativeEntry> {
    let mut out: Vec<NativeEntry> = rows
        .into_iter()
        .map(|row| {
            let tags = row.tags();
            let entry = NativeEntry::new(
                workspace_line(&row),
                workspace_detail(&row),
                NativeAction::Select(row.selection()),
                tags,
                row.group(),
            );
            match &row.session {
                Some(session) if !row.window_details.is_empty() => {
                    entry.with_windows(session.clone(), window_rows(&row))
                }
                _ => entry,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        a.group
            .order()
            .cmp(&b.group.order())
            .then_with(|| entry_sort_name(a).cmp(&entry_sort_name(b)))
    });
    out
}

/// The merged workspace list (all groups). Retained as a test oracle for the
/// shared collect/dedup logic; the picker uses the per-mode builders below.
#[cfg(test)]
pub(crate) fn build_native_entries(model: &MenuModel, snapshot: &TmuxSnapshot) -> Vec<NativeEntry> {
    rows_to_entries(collect_workspace_rows(model, snapshot))
}

/// Sessions mode: only running tmux sessions, each expandable into its window
/// tree. This is the honest per-session view — a multi-agent session shows a
/// state tally + count and never a fake single model.
pub(crate) fn build_session_entries(
    model: &MenuModel,
    snapshot: &TmuxSnapshot,
) -> Vec<NativeEntry> {
    rows_to_entries(
        collect_workspace_rows(model, snapshot)
            .into_iter()
            .filter(|row| row.session.is_some())
            .collect(),
    )
}

/// Layouts mode: the tmuxinator inventory (repo-local layouts + global
/// projects), each flagged running/stopped. Enter switches to a running one or
/// starts a stopped one — both handled by `execute`.
pub(crate) fn build_layout_entries(model: &MenuModel, snapshot: &TmuxSnapshot) -> Vec<NativeEntry> {
    let running: std::collections::HashSet<&str> =
        snapshot.sessions.iter().map(|s| s.name.as_str()).collect();
    let mut entries: Vec<NativeEntry> = Vec::new();

    let home = crate::discovery::home();

    let mut layouts = model.layouts.clone();
    layouts.sort_by(|a, b| a.session.cmp(&b.session));
    for l in &layouts {
        let is_running = l.running || running.contains(l.session.as_str());
        // Row shows the repo directory (its basename is the layout name, so it
        // survives a middle-ellipsis); the detail carries the exact yaml file.
        let yaml = crate::discovery::resolve_local_layout_file(&l.path);
        entries.push(layout_entry(
            &l.session,
            labels().kind_layout,
            is_running,
            Some((l.display.clone(), yaml)),
            Selection::Layout {
                session: l.session.clone(),
                full_path: l.path.clone(),
            },
        ));
    }
    for p in &model.projects {
        let is_running = running.contains(p.as_str());
        let yaml = crate::discovery::tmuxinator_project_file(p);
        let yaml_paths = std::path::Path::new(&yaml)
            .exists()
            .then(|| (crate::model::tilde(&yaml, &home), yaml));
        entries.push(layout_entry(
            p,
            labels().kind_project,
            is_running,
            yaml_paths,
            Selection::Project(p.clone()),
        ));
    }
    entries.sort_by_key(entry_sort_name);
    entries
}

/// Build one Layouts-mode row. When `yaml` is known, the row's name column shows
/// the tilde-collapsed path to the layout's yaml (middle-elided when it must
/// clip) and the detail pane carries the full absolute path; otherwise the row
/// falls back to the layout/project name.
fn layout_entry(
    name: &str,
    kind: &str,
    running: bool,
    yaml: Option<(String, String)>,
    selection: Selection,
) -> NativeEntry {
    let glyph = if running { GLYPHS.running } else { GLYPHS.idle };
    let status = if running {
        labels().status_running
    } else {
        labels().status_stopped
    };
    // Path in the (flexible) name column when we have one; else the name.
    let name_col = yaml.as_ref().map(|(disp, _)| disp.as_str()).unwrap_or(name);
    let line = format!("{glyph} {name_col} · {kind} · {status} · -");

    let mut detail = vec![
        "Layout".to_string(),
        format!("Name: {name}"),
        format!("Kind: {kind}"),
        format!("State: {status}"),
    ];
    if let Some((_, full)) = &yaml {
        detail.push(format!("Path: {full}"));
    }
    detail.push(format!(
        "Default action: {}",
        if running {
            "switch to running session"
        } else {
            "start this layout"
        }
    ));

    let entry = NativeEntry::new(
        line,
        detail.join("\n"),
        NativeAction::Select(selection),
        vec!["layout", "project"],
        NativeGroup::Configured,
    );
    if yaml.is_some() {
        entry.with_name_as_path()
    } else {
        entry
    }
}

/// Agents mode: one row per agent-pane across every session, so model and state
/// are never ambiguous. Grouped needs-you / working / quiet; Enter jumps
/// straight to the exact `session:window.pane`.
pub(crate) fn build_agent_entries(snapshot: &TmuxSnapshot) -> Vec<NativeEntry> {
    let mut entries: Vec<NativeEntry> = Vec::new();
    for session in &snapshot.sessions {
        for window in &session.windows {
            for pane in &window.panes {
                let Some(agent) = &pane.agent else { continue };
                let group = if agent.attention || agent.status.needs_attention() {
                    NativeGroup::AgentNeedsYou
                } else if agent.status == PaneAgentStatus::Working || agent.is_active {
                    NativeGroup::AgentWorking
                } else {
                    NativeGroup::AgentQuiet
                };
                let glyph = agent.status.glyph();
                let model = agent.model.as_deref().unwrap_or("?");
                let loc = format!("{}:{}", session.name, window.name);
                let last = relative_activity(agent.last_change.or(pane.last_activity));
                // columns: name(kind+loc) · model · status · last — model is the
                // whole point of this mode, so it gets its own column.
                let line = format!(
                    "{glyph} {} {loc} · {model} · {} · {last}",
                    agent.kind,
                    agent.status.short_label()
                );
                entries.push(NativeEntry::new(
                    line,
                    agent_detail(&session.name, window, pane, agent),
                    NativeAction::Select(Selection::Pane {
                        session: session.name.clone(),
                        window_id: window.id.clone(),
                        pane_id: pane.id.clone(),
                    }),
                    vec!["agent"],
                    group,
                ));
            }
        }
    }
    entries.sort_by(|a, b| {
        a.group
            .order()
            .cmp(&b.group.order())
            .then_with(|| entry_sort_name(a).cmp(&entry_sort_name(b)))
    });
    entries
}

fn agent_detail(session: &str, window: &TmuxWindow, pane: &TmuxPane, agent: &AgentState) -> String {
    let mut lines = vec![
        "Agent".to_string(),
        format!("Name: {} agent", agent.kind),
        format!("Model: {}", agent.model.as_deref().unwrap_or("unknown")),
        format!("State: {}", agent.status.as_str()),
        format!("Confidence: {}%", agent.confidence),
        format!("Location: {session}:{} ({})", window.name, pane.id),
        format!(
            "Last change: {}",
            relative_activity(agent.last_change.or(pane.last_activity))
        ),
    ];
    if !agent.wait_reason.is_empty() {
        lines.push(format!("Waiting: {}", agent.wait_reason));
    }
    if !agent.evidence.is_empty() {
        lines.push(format!("Evidence: {}", agent.evidence.join("; ")));
    }
    lines.join("\n")
}

pub(crate) fn build_directory_entries(model: &MenuModel) -> Vec<NativeEntry> {
    let mut seen = BTreeMap::<String, DirItem>::new();
    for dir in model.zoxide.iter().chain(model.plain_repos.iter()) {
        seen.entry(dir.path.clone()).or_insert_with(|| dir.clone());
    }

    let mut entries: Vec<NativeEntry> = seen
        .into_values()
        .map(|dir| {
            let caps = if dir.has_local_config {
                " "
            } else {
                ""
            };
            let activity = if dir.has_local_config {
                labels().dir_configured
            } else {
                labels().dir_bare
            };
            let detail = [
                "Directory",
                &format!("Path: {}", dir.path),
                if dir.has_local_config {
                    "Config: local tmuxinator config found"
                } else {
                    "Config: none found"
                },
                if dir.has_local_config {
                    "Default action: start local layout"
                } else {
                    "Default action: create bare tmux session"
                },
            ]
            .join("\n");
            NativeEntry::new(
                format!("{} {} · {caps} · {activity} · -", GLYPHS.idle, dir.display),
                detail,
                NativeAction::Select(Selection::Dir {
                    full_path: dir.path,
                    has_local_config: dir.has_local_config,
                }),
                vec!["dir"],
                NativeGroup::Directories,
            )
        })
        .collect();
    entries.sort_by_key(entry_sort_name);
    entries
}
