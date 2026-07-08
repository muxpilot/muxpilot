use serde::{Deserialize, Serialize};

/// A directory discovered by zoxide or the git-repo scan, plus whether it
/// carries a launchable tmuxinator layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirItem {
    /// Tilde-collapsed display path (e.g. `~/gits/github/org/repo`).
    pub display: String,
    /// Absolute filesystem path.
    pub path: String,
    /// True when the directory has a `.tmuxinator.yml` or `.agentvibes/tmux.yml`.
    pub has_local_config: bool,
}

/// A repo-local layout that can be started or switched to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    /// The tmux session name the layout will create.
    pub session: String,
    /// Tilde-collapsed display path of the repo.
    pub display: String,
    /// Absolute filesystem path of the repo.
    pub path: String,
    /// True when a session by this name is already running.
    pub running: bool,
}

/// Everything needed to render the menu.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MenuModel {
    pub sessions: Vec<SessionItem>,
    pub current: String,
    pub layouts: Vec<Layout>,
    pub projects: Vec<String>,
    pub zoxide: Vec<DirItem>,
    pub plain_repos: Vec<DirItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionItem {
    pub name: String,
    pub windows: u32,
}

/// The launchable action a menu line maps back to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// `🚀` repo-local layout: switch if up, else start from the repo.
    Layout { session: String, full_path: String },
    /// `📺` / `🟢` existing session: attach or switch-client.
    Session(String),
    /// `🎬` saved global tmuxinator project.
    Project(String),
    /// `⭐` / `📁` directory: launch its layout, or make a bare session.
    Dir {
        full_path: String,
        has_local_config: bool,
    },
    /// Native picker / CLI direct switch to a tmux window.
    Window { session: String, window_id: String },
    /// Native picker / CLI direct switch to a tmux pane.
    Pane {
        session: String,
        window_id: String,
        pane_id: String,
    },
}

/// Collapse a leading `$HOME` to `~` (first occurrence only, matching JS
/// `String.replace`).
pub fn tilde(path: &str, home: &str) -> String {
    if home.is_empty() {
        return path.to_string();
    }
    path.replacen(home, "~", 1)
}

/// Expand a leading `~` back to `$HOME` (first occurrence only).
pub fn untilde(path: &str, home: &str) -> String {
    if path.starts_with('~') {
        path.replacen('~', home, 1)
    } else {
        path.to_string()
    }
}

/// The tmux session name a bare directory gets: its basename with `.` -> `_`.
pub fn sanitize_session_name(path: &str) -> String {
    let base = path.rsplit('/').find(|s| !s.is_empty()).unwrap_or(path);
    let name = base.replace('.', "_");
    if name.is_empty() {
        "default".to_string()
    } else {
        name
    }
}

/// Turn a tmuxinator config filename into a project name, or `None` if it is a
/// non-project doc (`AGENTS.md` / `TEMPLATES.md` siblings, kept as `.yml` here).
pub fn tmuxinator_project_name(filename: &str) -> Option<String> {
    let name = filename.strip_suffix(".yml")?;
    let name = name.rsplit('/').next().unwrap_or(name);
    if name == "AGENTS" || name == "TEMPLATES" {
        return None;
    }
    Some(name.to_string())
}

/// Extract the `name:` value from a layout file's text. Returns `None` when no
/// `name:` line exists or the value is ERB-templated (`<%= ... %>`), so the
/// caller can fall back to the next candidate or the directory name.
pub fn parse_layout_name(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("name:") {
            let value = rest.trim();
            if value.is_empty() || value.contains("<%") {
                return None;
            }
            return Some(value.trim_matches(|c| c == '"' || c == '\'').to_string());
        }
    }
    None
}

/// Parse a chosen menu line back into the action to perform.
pub fn parse_selection(choice: &str, home: &str) -> Option<Selection> {
    if let Some(body) = choice.strip_prefix("🚀 ") {
        let body = body.strip_suffix(" ▶").unwrap_or(body);
        let (session, tilde_path) = body.split_once(" · ")?;
        return Some(Selection::Layout {
            session: session.to_string(),
            full_path: untilde(tilde_path, home),
        });
    }

    for prefix in ["📺 ", "🟢 "] {
        if let Some(name) = choice.strip_prefix(prefix) {
            let name = name.split_once(" · ").map(|(name, _)| name).unwrap_or(name);
            return Some(Selection::Session(name.trim().to_string()));
        }
    }

    if let Some(name) = choice.strip_prefix("🎬 ") {
        return Some(Selection::Project(name.trim().to_string()));
    }

    for prefix in ["⭐ ", "📁 "] {
        if let Some(rest) = choice.strip_prefix(prefix) {
            let (name, has_local_config) = match rest.strip_suffix(" 📄") {
                Some(stripped) => (stripped, true),
                None => (rest, false),
            };
            return Some(Selection::Dir {
                full_path: untilde(name.trim(), home),
                has_local_config,
            });
        }
    }

    None
}

/// Render the full menu (one line per item) in the script's section order:
/// sessions, repo-local layouts, global projects, zoxide dirs, plain repos.
pub fn build_menu_lines(model: &MenuModel) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for s in &model.sessions {
        let marker = if s.name == model.current {
            "🟢"
        } else {
            "📺"
        };
        let windows = match s.windows {
            1 => "1 window".to_string(),
            n => format!("{n} windows"),
        };
        out.push(format!("{marker} {} · {windows}", s.name));
    }

    let mut layouts = model.layouts.clone();
    layouts.sort_by(|a, b| a.session.cmp(&b.session));
    for l in &layouts {
        let running = if l.running { " ▶" } else { "" };
        out.push(format!("🚀 {} · {}{}", l.session, l.display, running));
    }

    for p in &model.projects {
        out.push(format!("🎬 {p}"));
    }

    for d in &model.zoxide {
        if d.has_local_config {
            out.push(format!("⭐ {} 📄", d.display));
        } else {
            out.push(format!("⭐ {}", d.display));
        }
    }

    for r in &model.plain_repos {
        out.push(format!("📁 {}", r.display));
    }

    out
}
