//! Hidden `muxpilot demo` command.
//!
//! Generates a deterministic set of fake workspaces (any count) and runs the
//! real native picker over them. Used to (a) stress-test truncation + filtering
//! with a huge inventory and (b) drive reproducible screenshots/videos without a
//! live tmux server. Not listed in `--help` or `commands` — it is a maintainer
//! tool. Selecting a row never launches anything.

use std::collections::HashSet;
use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal;

use crate::error::AppError;
use crate::model::Selection;
use crate::native_state::{
    FilterInput, NativeAction, NativeEntry, NativeGroup, SearchMode, WindowRow,
};
use crate::native_view::{
    apply_tree_key, draw_native_picker, selectable_rows, PickerScreen, PickerView, TreeKey,
};
use crate::ui::*;

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

const AGENT_KINDS: &[&str] = &["claude", "codex", "aider", "opencode"];

/// Tiny deterministic hash so the same `count` always yields the same data.
fn h(i: usize, salt: usize) -> usize {
    let mut x = (i.wrapping_mul(2654435761)).wrapping_add(salt.wrapping_mul(40503));
    x ^= x >> 13;
    x = x.wrapping_mul(1274126177);
    x ^= x >> 16;
    x
}

fn last_activity_label(i: usize) -> &'static str {
    match h(i, 7) % 8 {
        0 | 1 => "now",
        2 => "3m",
        3 => "21m",
        4 => "2h",
        5 => "19h",
        6 => "3d",
        _ => "5d",
    }
}

fn window_activity_label(i: usize, w: usize) -> &'static str {
    match h(i, 50 + w) % 8 {
        0 | 1 => "now",
        2 => "2m",
        3 => "14m",
        4 => "1h",
        5 => "6h",
        6 => "2d",
        _ => "4d",
    }
}

/// Deterministic fake windows for a running session. The first `agent_count`
/// windows carry an agent so the parent's `◍` count and the tree agree.
fn build_demo_windows(i: usize, windows: usize, agent_count: usize) -> Vec<WindowRow> {
    (0..windows)
        .map(|w| WindowRow {
            index: w as u32,
            id: format!("@{}", i * 10 + w),
            name: WINDOW_NAMES[h(i, 20 + w) % WINDOW_NAMES.len()].to_string(),
            active: w == 0,
            panes: 1 + h(i, 30 + w) % 3,
            agents: usize::from(w < agent_count),
            activity: window_activity_label(i, w).to_string(),
        })
        .collect()
}

/// Build `count` deterministic fake entries spanning running (with/without
/// agents) and configured groups, with widely varied name lengths.
pub(crate) fn build_demo_entries(count: usize) -> Vec<NativeEntry> {
    let mut entries = Vec::with_capacity(count);
    let mut used_names: HashSet<String> = HashSet::new();
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

        let running = !h(i, 2).is_multiple_of(5); // ~80% running
        let windows = 1 + h(i, 3) % 8;
        let agent_count = if running && !h(i, 4).is_multiple_of(3) {
            (1 + h(i, 5) % 4).min(windows)
        } else {
            0
        };
        let current = i == 0;
        let window_rows = if running {
            build_demo_windows(i, windows, agent_count)
        } else {
            Vec::new()
        };

        let glyph = if current {
            '◆'
        } else if running {
            '●'
        } else {
            '○'
        };

        let caps = if running {
            let mut c = format!("{windows}w");
            if agent_count > 0 {
                // Trailing space before the count: the ◍ glyph paints slightly
                // wider than one cell, so an adjacent digit would overlap it.
                c.push_str(&format!(" ◍ {agent_count}"));
            }
            c
        } else {
            String::new()
        };

        let status = if agent_count > 0 {
            "agent"
        } else if running {
            "active"
        } else {
            // Not "configured" — that just echoes the CONFIGURED group header.
            // Name the source (matches the real picker's status column).
            "tmuxinator"
        };

        let last = if running { last_activity_label(i) } else { "-" };
        let line = format!("{glyph} {name} · {caps} · {status} · {last}");

        let mut tags: Vec<&'static str> = Vec::new();
        let group = if running {
            tags.push("session");
            tags.push("window");
            NativeGroup::Running
        } else {
            tags.push("layout");
            tags.push("project");
            NativeGroup::Configured
        };
        if agent_count > 0 {
            tags.push("agent");
        }

        // A detail block shaped like the real one so the preview pane renders.
        let mut detail = vec![
            "Workspace".to_string(),
            format!("Name: {name}"),
            format!("State: {}", group.label().to_ascii_lowercase()),
            format!("Windows: {windows}"),
            format!("Last activity: {last}"),
            format!("Path: ~/code/{base}"),
        ];
        // List the windows so the preview shows them and filtering matches names.
        if !window_rows.is_empty() {
            detail.push("Windows".to_string());
            for win in &window_rows {
                let active = if win.active { "*" } else { " " };
                let agent = if win.agents == 0 {
                    String::new()
                } else {
                    format!("  ◍ {}", win.agents)
                };
                detail.push(format!(
                    "  {active} {}:{} {}  {}p{}  {}",
                    win.index, win.id, win.name, win.panes, agent, win.activity
                ));
            }
        }
        for a in 0..agent_count {
            let kind = AGENT_KINDS[h(i, 10 + a) % AGENT_KINDS.len()];
            detail.push(format!(
                "Agent: {kind}:working {}% pane %{a}",
                40 + h(i, a) % 60
            ));
        }

        let entry = NativeEntry::new(
            line,
            detail.join("\n"),
            NativeAction::Select(Selection::Session(name.clone())),
            tags,
            group,
        );
        let entry = if running {
            entry.with_windows(name, window_rows)
        } else {
            entry
        };
        entries.push(entry);
    }
    // Match the real picker's ordering: running group first, then by name.
    entries.sort_by(|a, b| {
        let order = |g: NativeGroup| match g {
            NativeGroup::Running => 0,
            NativeGroup::Configured => 1,
            NativeGroup::Directories => 2,
        };
        order(a.group)
            .cmp(&order(b.group))
            .then_with(|| entry_sort_name(a).cmp(&entry_sort_name(b)))
    });
    entries
}

/// Entry point for `muxpilot demo [--count N]`.
pub(crate) fn run_demo(count: usize) -> Result<ExitCode, AppError> {
    let entries = build_demo_entries(count);

    // Non-interactive (piped/CI): print the resolved rows and exit. Handy for
    // asserting truncation/filtering in tests without a PTY.
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        for entry in &entries {
            println!("{}", entry.line);
        }
        return Ok(ExitCode::SUCCESS);
    }

    let _guard = CrosstermGuard::enter()?;
    let mut filter = FilterInput::default();
    let mut cursor = 0usize;
    let mut mode = SearchMode::All;
    let mut show_help = false;
    let mut edit_filter = false;
    let mut theme = default_theme();
    let mut expanded: HashSet<String> = HashSet::new();

    loop {
        let filtered: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| entry_matches(entry, filter.text(), mode).then_some(idx))
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
                screen: PickerScreen::Main,
                show_help,
                edit_filter,
            },
            theme,
        )?;

        // Poll briefly so visible agent spinners animate in recordings.
        if !event::poll(Duration::from_millis(400))
            .map_err(|e| terminal_error("failed to poll key", e))?
        {
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
            KeyCode::Char('?') if !edit_filter => show_help = !show_help,
            KeyCode::Char('t') if !edit_filter && !show_help => theme = theme.toggled(),
            KeyCode::Char('q') if !edit_filter && !show_help => return Ok(ExitCode::SUCCESS),
            KeyCode::Tab if !show_help => {
                mode = mode.next();
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
                    TreeKey::Expand,
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
