use std::collections::HashSet;
use std::io::IsTerminal;
use std::time::Duration;

use crate::error::AppError;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal;

use crate::keymap::{Action, Keymap};
use crate::model::{build_menu_lines, parse_selection, MenuModel, Selection};
use crate::native_state::{FilterInput, NativeAction, NativeEntry, PickerMode, SearchMode};
use crate::native_view::{
    apply_tree_key, draw_native_picker, help_max_scroll, selectable_rows, visible_has_agent,
    PickerView, Selectable, TreeKey,
};
use crate::snapshot::{tmux_snapshot_with_options, SnapshotOptions, TmuxSnapshot};

/// The picker scrapes agent panes (gated to known agents) so it can show live
/// working/idle state and the active-now signal, refreshed on open and on `r`.
const PICKER_SNAPSHOT: SnapshotOptions = SnapshotOptions { capture_pane: true };
use crate::ui::*;
use crate::workspace_entries::{
    build_agent_entries, build_directory_entries, build_layout_entries, build_session_entries,
    fleet_summary,
};
use crate::{home, select_with_fzf};

/// Build the entry list for the active mode. Each mode is its own list built by
/// its own function — no shared merged view.
fn entries_for_mode(mode: PickerMode, model: &MenuModel, snapshot: &TmuxSnapshot) -> Vec<NativeEntry> {
    match mode {
        PickerMode::Sessions => build_session_entries(model, snapshot),
        PickerMode::Agents => build_agent_entries(snapshot),
        PickerMode::Layouts => build_layout_entries(model, snapshot),
        PickerMode::Dirs => build_directory_entries(model),
    }
}

/// The `Selection` the cursor currently points at, resolving through the tree
/// level (session / window / pane). Shared by Enter in both command and
/// filter-edit modes so they can never disagree.
fn selection_at(
    selectables: &[Selectable],
    filtered: &[usize],
    entries: &[NativeEntry],
    cursor: usize,
) -> Option<Selection> {
    match selectables.get(cursor)? {
        Selectable::Entry(pos) => {
            let entry = &entries[*filtered.get(*pos)?];
            match &entry.action {
                NativeAction::Select(selection) => Some(selection.clone()),
            }
        }
        Selectable::Window { pos, win } => {
            let entry = &entries[*filtered.get(*pos)?];
            let session = entry.session.clone()?;
            let window = entry.windows.get(*win)?;
            Some(Selection::Window {
                session,
                window_id: window.id.clone(),
            })
        }
        Selectable::Pane { pos, win, pane } => {
            let entry = &entries[*filtered.get(*pos)?];
            let session = entry.session.clone()?;
            let window = entry.windows.get(*win)?;
            let pane_row = window.pane_rows.get(*pane)?;
            Some(Selection::Pane {
                session,
                window_id: window.id.clone(),
                pane_id: pane_row.id.clone(),
            })
        }
    }
}

pub(crate) async fn select_native(model: &MenuModel) -> Result<Option<Selection>, AppError> {
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        let menu = build_menu_lines(model).join("\n");
        return Ok(select_with_fzf(&menu)
            .await?
            .and_then(|choice| parse_selection(&choice, &home())));
    }

    let _guard = CrosstermGuard::enter()?;
    let keymap = Keymap::defaults();
    let mut snapshot = tmux_snapshot_with_options(PICKER_SNAPSHOT);
    let mut mode = PickerMode::Sessions;
    let mut entries = entries_for_mode(mode, model, &snapshot);
    let mut fleet = fleet_summary(&snapshot);
    let mut filter = FilterInput::default();
    let mut cursor = 0usize;
    // On first render, home the cursor onto the current workspace (◆) instead of
    // the top row, so the picker opens on "where you are". Set once.
    let mut cursor_homed = false;
    let mut show_help = false;
    let mut help_scroll = 0usize;
    let mut edit_filter = false;
    let mut theme = default_theme();
    // Sessions expanded into their window children, keyed by session name so the
    // state survives filter changes and tmux-state refreshes.
    let mut expanded: HashSet<String> = HashSet::new();

    loop {
        let filtered: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                entry_matches(entry, filter.text(), SearchMode::All).then_some(idx)
            })
            .collect();
        let selectables = selectable_rows(&entries, &filtered, &expanded);
        if !cursor_homed {
            cursor_homed = true;
            // The current workspace carries the ◆ glyph as the first char of its line.
            if let Some(idx) = selectables.iter().position(|s| {
                matches!(s, Selectable::Entry(pos)
                    if entries[filtered[*pos]].line.starts_with('◆'))
            }) {
                cursor = idx;
            }
        }
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

        let animate_visible_agents = visible_has_agent(&entries, &filtered, &expanded, cursor);
        let poll_timeout = if animate_visible_agents {
            // Slightly oversample the 160ms spinner frame period so the braille
            // spinner animates smoothly rather than skipping frames.
            Duration::from_millis(120)
        } else {
            Duration::from_secs(3600)
        };
        if !event::poll(poll_timeout).map_err(|e| terminal_error("failed to poll key", e))? {
            // Timeout: rebuild entries from the cached snapshot (no re-scrape) so
            // the time-based agent spinner advances a frame, then redraw.
            entries = entries_for_mode(mode, model, &snapshot);
            continue;
        }
        let event = event::read().map_err(|e| terminal_error("failed to read key", e))?;
        let Event::Key(key) = event else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        // Ctrl-C quits from any sub-mode.
        if ctrl && key.code == KeyCode::Char('c') {
            return Ok(None);
        }

        // Filter-edit mode owns the keyboard: readline-style line editing.
        if edit_filter {
            match key.code {
                KeyCode::Esc => edit_filter = false,
                KeyCode::Enter => {
                    if let Some(sel) = selection_at(&selectables, &filtered, &entries, cursor) {
                        return Ok(Some(sel));
                    }
                }
                KeyCode::Char('u') if ctrl => {
                    filter.clear();
                    cursor = 0;
                }
                KeyCode::Char('w') if ctrl => {
                    filter.delete_word_before_cursor();
                    cursor = 0;
                }
                KeyCode::Char('a') if ctrl => filter.move_start(),
                KeyCode::Char('e') if ctrl => filter.move_end(),
                KeyCode::Char('b') if ctrl => filter.move_left(),
                KeyCode::Char('f') if ctrl => filter.move_right(),
                KeyCode::Left => filter.move_left(),
                KeyCode::Right => filter.move_right(),
                KeyCode::Backspace => {
                    filter.backspace();
                    cursor = 0;
                }
                KeyCode::Char(ch) if !ctrl => {
                    filter.insert(ch);
                    cursor = 0;
                }
                _ => {}
            }
            continue;
        }

        // Help overlay owns the keyboard: scrolling only.
        if show_help {
            let (_, rows) = terminal::size().unwrap_or((100, 30));
            let rows = rows as usize;
            let half = (picker_body_rows(rows) / 2).max(1);
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') => show_help = false,
                KeyCode::Down | KeyCode::Char('j') => {
                    help_scroll = (help_scroll + 1).min(help_max_scroll(rows));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    help_scroll = help_scroll.saturating_sub(1);
                }
                KeyCode::Char('g') => help_scroll = 0,
                KeyCode::Char('G') => help_scroll = help_max_scroll(rows),
                KeyCode::Char('d') if ctrl => {
                    help_scroll = (help_scroll + half).min(help_max_scroll(rows));
                }
                KeyCode::Char('u') if ctrl => help_scroll = help_scroll.saturating_sub(half),
                _ => {}
            }
            continue;
        }

        // Command mode: Esc closes the picker; every other key resolves through
        // the (reconfigurable) keymap to a semantic Action.
        if key.code == KeyCode::Esc {
            return Ok(None);
        }
        match keymap.resolve(key.code, key.modifiers) {
            Some(Action::Open) => {
                if let Some(sel) = selection_at(&selectables, &filtered, &entries, cursor) {
                    return Ok(Some(sel));
                }
            }
            Some(Action::Quit) => return Ok(None),
            Some(Action::Down) => {
                if !selectables.is_empty() {
                    cursor = (cursor + 1).min(selectables.len() - 1);
                }
            }
            Some(Action::Up) => cursor = cursor.saturating_sub(1),
            Some(Action::Top) => cursor = 0,
            Some(Action::Bottom) => {
                if !selectables.is_empty() {
                    cursor = selectables.len() - 1;
                }
            }
            Some(Action::PageDown) => {
                if !selectables.is_empty() {
                    let (_, rows) = terminal::size().unwrap_or((100, 30));
                    let step = (picker_body_rows(rows as usize) / 2).max(1);
                    cursor = (cursor + step).min(selectables.len() - 1);
                }
            }
            Some(Action::PageUp) => {
                let (_, rows) = terminal::size().unwrap_or((100, 30));
                let step = (picker_body_rows(rows as usize) / 2).max(1);
                cursor = cursor.saturating_sub(step);
            }
            Some(Action::ExpandLevel) => {
                cursor = apply_tree_key(
                    TreeKey::EntryToggle,
                    &selectables,
                    &entries,
                    &filtered,
                    &mut expanded,
                    cursor,
                );
            }
            Some(Action::ToggleLevel) => {
                cursor = apply_tree_key(
                    TreeKey::Toggle,
                    &selectables,
                    &entries,
                    &filtered,
                    &mut expanded,
                    cursor,
                );
            }
            Some(Action::CollapseLevel) => {
                cursor = apply_tree_key(
                    TreeKey::Collapse,
                    &selectables,
                    &entries,
                    &filtered,
                    &mut expanded,
                    cursor,
                );
            }
            Some(Action::SwitchMode(target)) => {
                if target != mode {
                    mode = target;
                    entries = entries_for_mode(mode, model, &snapshot);
                    filter.clear();
                    cursor = 0;
                }
            }
            Some(Action::NextMode) => {
                mode = mode.next();
                entries = entries_for_mode(mode, model, &snapshot);
                filter.clear();
                cursor = 0;
            }
            Some(Action::EditFilter) => edit_filter = true,
            Some(Action::ToggleHelp) => {
                show_help = true;
                help_scroll = 0;
            }
            Some(Action::ToggleTheme) => theme = theme.toggled(),
            Some(Action::Refresh) => {
                snapshot = tmux_snapshot_with_options(PICKER_SNAPSHOT);
                fleet = fleet_summary(&snapshot);
                entries = entries_for_mode(mode, model, &snapshot);
                cursor = 0;
            }
            None => {}
        }
    }
}
