//! Hidden `muxpilot demo` command.
//!
//! Generates a deterministic synthetic tmux world (any size) and runs the REAL
//! native picker over it — the same per-mode builders (`build_session_entries`,
//! `build_agent_entries`, `build_layout_entries`, `build_directory_entries`) the
//! interactive picker drives through `entries_for_mode`. Used to (a) stress-test
//! truncation + filtering with a huge inventory and (b) drive reproducible
//! screenshots/videos without a live tmux server. Not listed in `--help` or
//! `commands` — it is a maintainer tool. Selecting a row never launches anything.
//!
//! An earlier version faked modes by filtering one merged list, so recordings
//! showed the wrong grouping (a TMUXINATOR group under Sessions, mis-grouped
//! Agents). This version builds a synthetic `(MenuModel, TmuxSnapshot)` and lets
//! the real builders group it, so the demo is faithful: Sessions is running-only,
//! Agents is grouped by attention, Layouts holds the tmuxinator files.

use std::collections::HashSet;
use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal;

use crate::error::AppError;
use crate::model::{DirItem, Layout, MenuModel};
use crate::native_picker::entries_for_mode;
use crate::native_state::{FilterInput, PickerMode};
use crate::native_view::{
    apply_tree_key, draw_native_picker, help_max_scroll, selectable_rows, visible_has_agent,
    PickerView, TreeKey,
};
use crate::snapshot::{
    AgentState, AgentStateSource, PaneAgentStatus, TmuxPane, TmuxSession, TmuxSnapshot, TmuxWindow,
};
use crate::ui::*;
use crate::workspace_entries::fleet_summary;

/// Invented, generic project names — nothing tied to any real setup. Deliberately
/// includes very long entries so the ellipsis/truncation path is always
/// exercised, plus a one-character name for the opposite extreme.
const BASES: &[&str] = &[
    "web-dashboard",
    "api-gateway",
    "payments-service",
    "mobile-app",
    "design-system",
    "auth-service",
    "data-pipeline",
    "notification-worker",
    "search-indexer",
    "billing-portal",
    "image-processing-batch-render-service-with-a-very-long-name",
    "cms-backend",
    "docs-site",
    "ml-feature-store",
    "email-campaign-scheduler",
    "infra-terraform-modules",
    "a",
    "customer-support-chatbot-training-pipeline-orchestrator-that-should-truncate",
];

/// tmux window names typical of a working session.
const WINDOW_NAMES: &[&str] = &[
    "editor", "server", "shell", "logs", "tests", "git", "build", "repl", "db", "watch", "agent",
    "notes",
];

const AGENT_KINDS: &[&str] = &["claude", "codex", "cline", "aider", "opencode"];
const MODELS: &[&str] = &["opus-4-8", "sonnet-5", "gpt-5.4", "haiku-4-5"];
const PANE_CMDS: &[&str] = &["zsh", "nvim", "cargo", "node", "git", "htop"];
const DEMO_STATUSES: &[PaneAgentStatus] = &[
    PaneAgentStatus::Working,
    PaneAgentStatus::WaitingApprove,
    PaneAgentStatus::WaitingInput,
    PaneAgentStatus::Idle,
];

/// Tiny deterministic hash so the same `count` always yields the same data.
fn h(i: usize, salt: usize) -> usize {
    let mut x = (i.wrapping_mul(2654435761)).wrapping_add(salt.wrapping_mul(40503));
    x ^= x >> 13;
    x = x.wrapping_mul(1274126177);
    x ^= x >> 16;
    x
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A deterministic "seconds ago" spread so windows/panes render varied
/// `now`/`3m`/`2h`/… activity labels in recordings.
fn activity_at(now: u64, i: usize, salt: usize) -> Option<u64> {
    let ago = match h(i, salt) % 8 {
        0 | 1 => 20,
        2 => 3 * 60,
        3 => 21 * 60,
        4 => 2 * 3600,
        5 => 19 * 3600,
        6 => 3 * 86_400,
        _ => 5 * 86_400,
    };
    Some(now.saturating_sub(ago))
}

/// A deterministic agent for the first pane of an agent-bearing window. Statuses
/// cycle through the full ladder so all three Agents groups (NEEDS YOU / WORKING
/// / QUIET) and the fleet tally are populated.
fn demo_agent(now: u64, i: usize, w: usize) -> AgentState {
    let status = DEMO_STATUSES[h(i, 7 + w) % DEMO_STATUSES.len()];
    let kind = AGENT_KINDS[h(i, 40 + w) % AGENT_KINDS.len()];
    let model = MODELS[h(i, 50 + w) % MODELS.len()];
    AgentState {
        kind: kind.to_string(),
        status,
        source: AgentStateSource::Process,
        confidence: (40 + h(i, 8 + w) % 60) as u8,
        attention: status.needs_attention(),
        wait_reason: if status.needs_attention() {
            "awaiting your input".to_string()
        } else {
            String::new()
        },
        model: Some(model.to_string()),
        evidence: vec!["process".to_string()],
        is_active: status == PaneAgentStatus::Working,
        last_change: activity_at(now, i, 7 + w),
    }
}

/// Deterministic fake windows for a running session. The first `agent_count`
/// windows carry an agent in their first pane so the parent's `◍` count and the
/// tree agree; the rest are plain shells, and multi-pane windows exercise the
/// third tree level with real-looking data.
fn demo_windows(now: u64, i: usize, windows: usize, agent_count: usize) -> Vec<TmuxWindow> {
    (0..windows)
        .map(|w| {
            let panes = 1 + h(i, 30 + w) % 3;
            let has_agent = w < agent_count;
            let pane_rows = (0..panes)
                .map(|p| {
                    let is_agent = has_agent && p == 0;
                    TmuxPane {
                        id: format!("%{}", i * 100 + w * 10 + p),
                        active: w == 0 && p == 0,
                        path: format!("/home/user/code/{}", BASES[h(i, 1) % BASES.len()]),
                        current_command: if is_agent {
                            AGENT_KINDS[h(i, 40 + w) % AGENT_KINDS.len()].to_string()
                        } else {
                            PANE_CMDS[h(i, 60 + w + p) % PANE_CMDS.len()].to_string()
                        },
                        pid: Some(1000 + (i * 100 + w * 10 + p) as u32),
                        last_activity: activity_at(now, i, 50 + w),
                        role: String::new(),
                        agent: is_agent.then(|| demo_agent(now, i, w)),
                    }
                })
                .collect();
            TmuxWindow {
                id: format!("@{}", i * 10 + w),
                index: w as u32,
                name: WINDOW_NAMES[h(i, 20 + w) % WINDOW_NAMES.len()].to_string(),
                active: w == 0,
                last_activity: activity_at(now, i, 50 + w),
                panes: pane_rows,
            }
        })
        .collect()
}

/// Build a deterministic synthetic world: `count` workspaces (~80% running).
/// Running ones become live tmux sessions in the snapshot; every workspace also
/// gets a tmuxinator layout (flagged running/stopped) and a zoxide/plain-repo
/// directory, so all four picker modes are populated. This is what the real
/// per-mode builders then group — the demo never hand-rolls entries.
pub(crate) fn build_demo_snapshot(count: usize) -> (MenuModel, TmuxSnapshot) {
    let now = now_secs();
    let mut used_names: HashSet<String> = HashSet::new();
    let mut sessions: Vec<TmuxSession> = Vec::new();
    let mut layouts: Vec<Layout> = Vec::new();
    let mut zoxide: Vec<DirItem> = Vec::new();
    let mut plain_repos: Vec<DirItem> = Vec::new();

    let mut current_session = String::new();
    let mut current_window_id = String::new();
    let mut current_pane_id = String::new();

    for i in 0..count {
        let base = BASES[h(i, 1) % BASES.len()];
        // Names must be unique — real tmux session names are, and expansion is
        // keyed by name, so a collision would expand two rows at once. Keep the
        // first use of a base bare and disambiguate later collisions with `-i`.
        let mut name = base.to_string();
        if !used_names.insert(name.clone()) {
            name = format!("{base}-{i}");
            used_names.insert(name.clone());
        }
        // Paths key off the unique session name (not the shared base) so distinct
        // workspaces never collapse to an identical-looking layout/dir row.
        let display = format!("~/code/{name}");
        let path = format!("/home/user/code/{name}");
        let running = !h(i, 2).is_multiple_of(5); // ~80% running

        // Every workspace has a launchable layout; `running` mirrors whether a
        // live session exists, exactly as discovery reports it.
        layouts.push(Layout {
            session: name.clone(),
            display: display.clone(),
            path: path.clone(),
            running,
        });

        if running {
            let windows = 1 + h(i, 3) % 8;
            let agent_count = if !h(i, 4).is_multiple_of(3) {
                (1 + h(i, 5) % 4).min(windows)
            } else {
                0
            };
            let window_rows = demo_windows(now, i, windows, agent_count);
            // The first running session is "current" (carries the ◆ glyph).
            if current_session.is_empty() {
                current_session = name.clone();
                if let Some(win) = window_rows.first() {
                    current_window_id = win.id.clone();
                    if let Some(pane) = win.panes.first() {
                        current_pane_id = pane.id.clone();
                    }
                }
            }
            sessions.push(TmuxSession {
                name: name.clone(),
                windows: window_rows,
            });
        }

        // Directories (Dirs mode): keep the set modest but varied; the builder
        // dedups by path.
        if zoxide.len() < 40 {
            zoxide.push(DirItem {
                display: display.clone(),
                path: path.clone(),
                has_local_config: h(i, 9).is_multiple_of(3),
            });
        }
        if plain_repos.len() < 12 {
            plain_repos.push(DirItem {
                display: format!("~/gits/github/acme/{name}"),
                path: format!("/home/user/gits/github/acme/{name}"),
                has_local_config: false,
            });
        }
    }

    let model = MenuModel {
        sessions: Vec::new(),
        current: current_session.clone(),
        layouts,
        projects: Vec::new(),
        zoxide,
        plain_repos,
    };
    let snapshot = TmuxSnapshot {
        schema_version: 1,
        source: "synthetic",
        backend: "synthetic",
        current_session,
        current_window_id,
        current_pane_id,
        sessions,
    };
    (model, snapshot)
}

/// Entry point for `muxpilot demo [--count N]`.
pub(crate) fn run_demo(count: usize) -> Result<ExitCode, AppError> {
    let (model, snapshot) = build_demo_snapshot(count);

    // Non-interactive (piped/CI): print each mode's resolved rows and exit. Handy
    // for eyeballing per-mode grouping without a PTY.
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        for mode in [
            PickerMode::Sessions,
            PickerMode::Agents,
            PickerMode::Layouts,
            PickerMode::Dirs,
        ] {
            println!("# {mode:?}");
            for entry in entries_for_mode(mode, &model, &snapshot) {
                println!("{}", entry.line);
            }
        }
        return Ok(ExitCode::SUCCESS);
    }

    let _guard = CrosstermGuard::enter()?;
    let mut mode = PickerMode::Sessions;
    let mut entries = entries_for_mode(mode, &model, &snapshot);
    // Real fleet counts, straight from the synthetic snapshot's agent panes.
    let fleet = fleet_summary(&snapshot);
    let mut filter = FilterInput::default();
    let mut cursor = 0usize;
    let mut show_help = false;
    let mut help_scroll = 0usize;
    let mut edit_filter = false;
    let mut theme = default_theme();
    let mut expanded: HashSet<String> = HashSet::new();

    loop {
        let filtered: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| entry_matches(entry, filter.text()).then_some(idx))
            .collect();
        let selectables = selectable_rows(&entries, &filtered, &expanded);
        if cursor >= selectables.len() {
            cursor = selectables.len().saturating_sub(1);
        }
        draw_native_picker(
            &entries,
            &filtered,
            &expanded,
            cursor,
            PickerView {
                filter: &filter,
                mode,
                show_help,
                help_scroll,
                edit_filter,
                fleet,
            },
            theme,
        )?;

        // Oversample the spinner frame period while an agent row is visible so the
        // braille spinner animates smoothly; otherwise idle until a keypress.
        let poll_timeout = if visible_has_agent(&entries, &filtered, &expanded, cursor) {
            Duration::from_millis(120)
        } else {
            Duration::from_secs(3600)
        };
        if !event::poll(poll_timeout).map_err(|e| terminal_error("failed to poll key", e))? {
            // Timeout: rebuild entries so the time-based agent spinner advances.
            entries = entries_for_mode(mode, &model, &snapshot);
            continue;
        }
        let Event::Key(key) = event::read().map_err(|e| terminal_error("failed to read key", e))?
        else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Char('c') if ctrl => return Ok(ExitCode::SUCCESS),
            KeyCode::Char('u') if ctrl && edit_filter && !show_help => {
                filter.clear();
                cursor = 0;
            }
            KeyCode::Char('w') if ctrl && edit_filter && !show_help => {
                filter.delete_word_before_cursor();
                cursor = 0;
            }
            KeyCode::Esc => {
                if show_help {
                    show_help = false;
                } else if edit_filter {
                    edit_filter = false;
                } else {
                    return Ok(ExitCode::SUCCESS);
                }
            }
            KeyCode::Backspace if edit_filter && !show_help => {
                filter.backspace();
                cursor = 0;
            }
            KeyCode::Char(ch) if edit_filter && !show_help && !ctrl => {
                filter.insert(ch);
                cursor = 0;
            }
            KeyCode::Char('?') if !edit_filter => {
                show_help = !show_help;
                help_scroll = 0;
            }
            KeyCode::Down | KeyCode::Char('j') if show_help => {
                let (_, rows) = terminal::size().unwrap_or((100, 30));
                help_scroll = (help_scroll + 1).min(help_max_scroll(rows as usize));
            }
            KeyCode::Up | KeyCode::Char('k') if show_help => {
                help_scroll = help_scroll.saturating_sub(1);
            }
            KeyCode::Char('t') if !edit_filter && !show_help => theme = theme.toggled(),
            KeyCode::Char('q') if !edit_filter && !show_help => return Ok(ExitCode::SUCCESS),
            KeyCode::Tab if !show_help => {
                mode = mode.next();
                entries = entries_for_mode(mode, &model, &snapshot);
                filter.clear();
                cursor = 0;
            }
            // Tab-bar letters: jump straight to a mode (mirrors the real picker's
            // s/a/x/d switch keys) so recordings can show the tabs in action.
            KeyCode::Char('s') if !edit_filter && !show_help && !ctrl => {
                mode = PickerMode::Sessions;
                entries = entries_for_mode(mode, &model, &snapshot);
                filter.clear();
                cursor = 0;
            }
            KeyCode::Char('a') if !edit_filter && !show_help && !ctrl => {
                mode = PickerMode::Agents;
                entries = entries_for_mode(mode, &model, &snapshot);
                filter.clear();
                cursor = 0;
            }
            KeyCode::Char('x') if !edit_filter && !show_help && !ctrl => {
                mode = PickerMode::Layouts;
                entries = entries_for_mode(mode, &model, &snapshot);
                filter.clear();
                cursor = 0;
            }
            KeyCode::Char('d') if !ctrl && !edit_filter && !show_help => {
                mode = PickerMode::Dirs;
                entries = entries_for_mode(mode, &model, &snapshot);
                filter.clear();
                cursor = 0;
            }
            KeyCode::Char('/') if !show_help => edit_filter = true,
            KeyCode::Down | KeyCode::Char('j') if !show_help && !selectables.is_empty() => {
                cursor = (cursor + 1).min(selectables.len() - 1);
            }
            KeyCode::Up | KeyCode::Char('k') if !show_help => cursor = cursor.saturating_sub(1),
            KeyCode::Char('g') if !edit_filter && !show_help => cursor = 0,
            KeyCode::Char('G') if !edit_filter && !show_help && !selectables.is_empty() => {
                cursor = selectables.len() - 1;
            }
            KeyCode::Char('d') if ctrl && !edit_filter && !show_help && !selectables.is_empty() => {
                let (_, rows) = terminal::size().unwrap_or((100, 30));
                let step = (picker_body_rows(rows as usize) / 2).max(1);
                cursor = (cursor + step).min(selectables.len() - 1);
            }
            KeyCode::Char('u') if ctrl && !edit_filter && !show_help => {
                let (_, rows) = terminal::size().unwrap_or((100, 30));
                let step = (picker_body_rows(rows as usize) / 2).max(1);
                cursor = cursor.saturating_sub(step);
            }
            // Tree: expand a running session into its windows / collapse it.
            KeyCode::Char(' ') if !edit_filter && !show_help => {
                cursor = apply_tree_key(
                    TreeKey::Toggle,
                    &selectables,
                    &entries,
                    &filtered,
                    &mut expanded,
                    cursor,
                );
            }
            KeyCode::Char('l') | KeyCode::Right if !edit_filter && !show_help => {
                cursor = apply_tree_key(
                    TreeKey::EntryToggle,
                    &selectables,
                    &entries,
                    &filtered,
                    &mut expanded,
                    cursor,
                );
            }
            KeyCode::Char('h') | KeyCode::Left if !edit_filter && !show_help => {
                cursor = apply_tree_key(
                    TreeKey::Collapse,
                    &selectables,
                    &entries,
                    &filtered,
                    &mut expanded,
                    cursor,
                );
            }
            KeyCode::Enter if !show_help => {
                // Demo never launches anything (session or window); just close.
                return Ok(ExitCode::SUCCESS);
            }
            _ => {}
        }
    }
}
