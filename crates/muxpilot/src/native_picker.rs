use std::collections::HashSet;
use std::io::IsTerminal;
use std::time::Duration;

use crate::error::AppError;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal;

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

pub(crate) async fn select_native(model: &MenuModel) -> Result<Option<Selection>, AppError> {
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        let menu = build_menu_lines(model).join("\n");
        return Ok(select_with_fzf(&menu)
            .await?
            .and_then(|choice| parse_selection(&choice, &home())));
    }

    let _guard = CrosstermGuard::enter()?;
    let mut snapshot = tmux_snapshot_with_options(PICKER_SNAPSHOT);
    let mut mode = PickerMode::Sessions;
    let mut entries = entries_for_mode(mode, model, &snapshot);
    let mut fleet = fleet_summary(&snapshot);
    let mut filter = FilterInput::default();
    let mut cursor = 0usize;
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
            Duration::from_millis(500)
        } else {
            Duration::from_secs(3600)
        };
        if !event::poll(poll_timeout).map_err(|e| terminal_error("failed to poll key", e))? {
            // Timeout is used only to advance visible agent spinners.
            continue;
        }
        let event = event::read().map_err(|e| terminal_error("failed to read key", e))?;
        let Event::Key(key) = event else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(None);
            }
            KeyCode::Char('u')
                if key.modifiers.contains(KeyModifiers::CONTROL) && edit_filter && !show_help =>
            {
                filter.clear();
                cursor = 0;
            }
            KeyCode::Char('w')
                if key.modifiers.contains(KeyModifiers::CONTROL) && edit_filter && !show_help =>
            {
                filter.delete_word_before_cursor();
                cursor = 0;
            }
            KeyCode::Char('a')
                if key.modifiers.contains(KeyModifiers::CONTROL) && edit_filter && !show_help =>
            {
                filter.move_start();
            }
            KeyCode::Char('e')
                if key.modifiers.contains(KeyModifiers::CONTROL) && edit_filter && !show_help =>
            {
                filter.move_end();
            }
            KeyCode::Left if edit_filter && !show_help => {
                filter.move_left();
            }
            KeyCode::Right if edit_filter && !show_help => {
                filter.move_right();
            }
            KeyCode::Char('b')
                if key.modifiers.contains(KeyModifiers::CONTROL) && edit_filter && !show_help =>
            {
                filter.move_left();
            }
            KeyCode::Char('f')
                if key.modifiers.contains(KeyModifiers::CONTROL) && edit_filter && !show_help =>
            {
                filter.move_right();
            }
            KeyCode::Esc => {
                if show_help {
                    show_help = false;
                } else if edit_filter {
                    edit_filter = false;
                } else {
                    return Ok(None);
                }
            }
            KeyCode::Backspace if edit_filter && !show_help => {
                filter.backspace();
                cursor = 0;
            }
            KeyCode::Char(ch)
                if edit_filter && !show_help && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                filter.insert(ch);
                cursor = 0;
            }
            KeyCode::Char('?') if !edit_filter => {
                show_help = !show_help;
                help_scroll = 0;
            }
            // Scroll the help overlay so long help is reachable on short terminals.
            KeyCode::Down | KeyCode::Char('j') if show_help => {
                let (_, rows) = terminal::size().unwrap_or((100, 30));
                help_scroll = (help_scroll + 1).min(help_max_scroll(rows as usize));
            }
            KeyCode::Up | KeyCode::Char('k') if show_help => {
                help_scroll = help_scroll.saturating_sub(1);
            }
            KeyCode::Char('g') if show_help => help_scroll = 0,
            KeyCode::Char('G') if show_help => {
                let (_, rows) = terminal::size().unwrap_or((100, 30));
                help_scroll = help_max_scroll(rows as usize);
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) && show_help => {
                let (_, rows) = terminal::size().unwrap_or((100, 30));
                let step = (picker_body_rows(rows as usize) / 2).max(1);
                help_scroll = (help_scroll + step).min(help_max_scroll(rows as usize));
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) && show_help => {
                help_scroll = help_scroll.saturating_sub({
                    let (_, rows) = terminal::size().unwrap_or((100, 30));
                    (picker_body_rows(rows as usize) / 2).max(1)
                });
            }
            KeyCode::Char('t') if !edit_filter && !show_help => theme = theme.toggled(),
            KeyCode::Char('q') if !edit_filter && !show_help => return Ok(None),
            KeyCode::Tab if !show_help => {
                mode = mode.next();
                entries = entries_for_mode(mode, model, &snapshot);
                filter.clear();
                cursor = 0;
            }
            KeyCode::Char('/') if !show_help => edit_filter = true,
            KeyCode::Char('r') if !edit_filter && !show_help => {
                snapshot = tmux_snapshot_with_options(PICKER_SNAPSHOT);
                fleet = fleet_summary(&snapshot);
                entries = entries_for_mode(mode, model, &snapshot);
                cursor = 0;
            }
            // Footer command keys `s`/`a`/`x`/`d` jump straight to a mode.
            KeyCode::Char(ch)
                if !edit_filter
                    && !show_help
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && PickerMode::from_key(ch).is_some_and(|m| m != mode) =>
            {
                mode = PickerMode::from_key(ch).unwrap();
                entries = entries_for_mode(mode, model, &snapshot);
                filter.clear();
                cursor = 0;
            }
            KeyCode::Down | KeyCode::Char('j') if !show_help && !selectables.is_empty() => {
                cursor = (cursor + 1).min(selectables.len() - 1);
            }
            KeyCode::Up | KeyCode::Char('k') if !show_help => {
                cursor = cursor.saturating_sub(1);
            }
            KeyCode::Char('g') if !edit_filter && !show_help => {
                cursor = 0;
            }
            KeyCode::Char('G') if !edit_filter && !show_help && !selectables.is_empty() => {
                cursor = selectables.len() - 1;
            }
            KeyCode::Char('d')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !edit_filter
                    && !show_help
                    && !selectables.is_empty() =>
            {
                let (_, rows) = terminal::size().unwrap_or((100, 30));
                let step = (picker_body_rows(rows as usize) / 2).max(1);
                cursor = (cursor + step).min(selectables.len() - 1);
            }
            KeyCode::Char('u')
                if key.modifiers.contains(KeyModifiers::CONTROL) && !edit_filter && !show_help =>
            {
                let (_, rows) = terminal::size().unwrap_or((100, 30));
                let step = (picker_body_rows(rows as usize) / 2).max(1);
                cursor = cursor.saturating_sub(step);
            }
            // Tree: Space and l/→ both toggle the selected session open/closed
            // into its windows; h/← collapses.
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
            KeyCode::Enter if !show_help => match selectables.get(cursor) {
                Some(Selectable::Entry(pos)) => {
                    if let Some(entry_idx) = filtered.get(*pos) {
                        match &entries[*entry_idx].action {
                            NativeAction::Select(selection) => return Ok(Some(selection.clone())),
                        }
                    }
                }
                Some(Selectable::Window { pos, win }) => {
                    if let Some(entry) = filtered.get(*pos).map(|idx| &entries[*idx]) {
                        if let (Some(session), Some(window)) =
                            (entry.session.clone(), entry.windows.get(*win))
                        {
                            return Ok(Some(Selection::Window {
                                session,
                                window_id: window.id.clone(),
                            }));
                        }
                    }
                }
                // A pane leaf jumps straight to that exact pane.
                Some(Selectable::Pane { pos, win, pane }) => {
                    if let Some(entry) = filtered.get(*pos).map(|idx| &entries[*idx]) {
                        if let (Some(session), Some(window)) =
                            (entry.session.clone(), entry.windows.get(*win))
                        {
                            if let Some(pane_row) = window.pane_rows.get(*pane) {
                                return Ok(Some(Selection::Pane {
                                    session,
                                    window_id: window.id.clone(),
                                    pane_id: pane_row.id.clone(),
                                }));
                            }
                        }
                    }
                }
                None => {}
            },
            _ => {}
        }
    }
}
