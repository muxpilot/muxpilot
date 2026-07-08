use std::collections::{BTreeMap, HashMap};
use std::time::SystemTime;

use crate::model::{DirItem, Layout, MenuModel, Selection};
use crate::native_state::{NativeAction, NativeEntry, NativeGroup};
use crate::snapshot::TmuxSnapshot;
use crate::ui::{entry_sort_name, spinner_frame};

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
    pub(crate) last_activity: Option<u64>,
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
        icons.push(format!("◍ {}", row.agents.len()));
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
    if row.agent_attention {
        return " wait".to_string();
    }
    if !row.agents.is_empty() {
        return format!("{} agent", spinner_frame());
    }
    if row.session.is_some() {
        return "active".to_string();
    }
    if row.layout.is_some() || row.project.is_some() {
        // The group header already says CONFIGURED — name the source instead of
        // echoing it, so the status column tells the user *where* the config
        // comes from (a tmuxinator layout/project).
        return "tmuxinator".to_string();
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
    if let Some(path) = &row.path {
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
        "◆"
    } else if row.session.is_some() {
        "●"
    } else {
        "○"
    };
    format!(
        "{state} {} · {} · {} · {}",
        row.name,
        capability_icons(row),
        workspace_activity(row),
        relative_activity(row.last_activity)
    )
}

pub(crate) fn build_native_entries(model: &MenuModel, snapshot: &TmuxSnapshot) -> Vec<NativeEntry> {
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
            });
            for pane in &window.panes {
                row.last_activity = row.last_activity.max(pane.last_activity);
                if row.path.is_none() && !pane.path.is_empty() {
                    row.path = Some(pane.path.clone());
                }
                if let Some(agent) = &pane.agent {
                    row.agents.push(format!(
                        "{}:{} {}% pane {}",
                        agent.kind, agent.status, agent.confidence, pane.id
                    ));
                    row.agent_attention |= agent.attention;
                }
            }
        }
    }

    let mut out: Vec<NativeEntry> = rows
        .into_values()
        .filter(|row| {
            row.session.is_some()
                || row.layout.is_some()
                || row.project.is_some()
                || row.dir.is_some()
        })
        .map(|row| {
            let tags = row.tags();
            let entry = NativeEntry::new(
                workspace_line(&row),
                workspace_detail(&row),
                NativeAction::Select(row.selection()),
                tags,
                row.group(),
            );
            // Running sessions carry their windows so the row can expand.
            match &row.session {
                Some(session) if !row.window_details.is_empty() => {
                    entry.with_windows(session.clone(), window_rows(&row))
                }
                _ => entry,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        let group_order = |group: NativeGroup| match group {
            NativeGroup::Running => 0,
            NativeGroup::Configured => 1,
            NativeGroup::Directories => 2,
        };
        group_order(a.group)
            .cmp(&group_order(b.group))
            .then_with(|| entry_sort_name(a).cmp(&entry_sort_name(b)))
    });
    out
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
                "configured"
            } else {
                "bare"
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
                format!("○ {} · {caps} · {activity} · -", dir.display),
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
