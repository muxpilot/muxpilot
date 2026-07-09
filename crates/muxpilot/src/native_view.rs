use std::collections::HashSet;
use std::io::Write;

use crate::error::{AppError, ErrorCode};
use crossterm::terminal;

use crate::native_state::{native_help_body, FilterInput, NativeEntry, NativeGroup, PickerMode};
use crate::ui::*;

/// Fixed left gutter before the column area: `marker + space + glyph + space`.
const PREFIX: usize = 4;
/// Trailing columns kept clear on the right so the rightmost value never sits in
/// the terminal's final cell — the row background still fills edge-to-edge, but
/// text is inset. Keeps last-activity readable and avoids terminal/recorder
/// edge-cell clipping.
const RIGHT_GUTTER: usize = 1;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PickerView<'a> {
    pub(crate) filter: &'a FilterInput,
    pub(crate) mode: PickerMode,
    pub(crate) show_help: bool,
    /// Scroll offset (in body lines) for the help overlay, so long help is
    /// reachable on short terminals.
    pub(crate) help_scroll: usize,
    pub(crate) edit_filter: bool,
    /// Fleet-wide agent counts for the status bar (T4).
    pub(crate) fleet: crate::workspace_entries::FleetSummary,
}

/// One rendered body line: a group header, a workspace row, or an expanded
/// window child of the workspace row above it.
enum DisplayRow {
    Header(&'static str),
    /// Index into the `filtered` slice.
    Entry(usize),
    /// A window child: `pos` is the parent's index into `filtered`, `win` is the
    /// index into that entry's `windows`.
    Window { pos: usize, win: usize },
    /// A pane leaf (third level): `pos`/`win` locate the parent window, `pane`
    /// indexes into that window's `pane_rows`.
    Pane { pos: usize, win: usize, pane: usize },
}

/// A navigable row — everything the cursor can land on (headers excluded). The
/// cursor is an index into the ordered list this produces.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Selectable {
    Entry(usize),
    Window { pos: usize, win: usize },
    Pane { pos: usize, win: usize, pane: usize },
}

/// Whether the session entry at filtered position `pos` is expanded into windows.
fn is_expanded(entry: &NativeEntry, expanded: &HashSet<String>) -> bool {
    entry.is_expandable()
        && entry
            .session
            .as_ref()
            .is_some_and(|s| expanded.contains(s))
}

/// The pane indices to show beneath a window row, given the expand set.
///
/// A one-pane window shows its agent inline on the row (no pane leaf). A
/// multi-pane window auto-reveals just its *agent* panes so their agent/model/
/// status is visible the moment the session is opened — no second expand; a
/// fully expanded window (its id in `expanded`) additionally shows every plain
/// shell pane.
fn displayed_pane_indices(
    win: &crate::native_state::WindowRow,
    expanded: &HashSet<String>,
) -> Vec<usize> {
    if !win.is_expandable() {
        return Vec::new();
    }
    if expanded.contains(&win.id) {
        return (0..win.pane_rows.len()).collect();
    }
    win.pane_rows
        .iter()
        .enumerate()
        .filter(|(_, pane)| pane.agent)
        .map(|(idx, _)| idx)
        .collect()
}

/// The ordered navigable rows (entries + the window children of expanded
/// sessions). `cursor` indexes into this; callers use it for Enter, expand, and
/// clamping. Kept in lockstep with [`build_display_rows`] (same non-header order).
pub(crate) fn selectable_rows(
    entries: &[NativeEntry],
    filtered: &[usize],
    expanded: &HashSet<String>,
) -> Vec<Selectable> {
    let mut out = Vec::with_capacity(filtered.len());
    for (pos, entry_idx) in filtered.iter().enumerate() {
        out.push(Selectable::Entry(pos));
        let entry = &entries[*entry_idx];
        if is_expanded(entry, expanded) {
            for (win, window) in entry.windows.iter().enumerate() {
                out.push(Selectable::Window { pos, win });
                for pane in displayed_pane_indices(window, expanded) {
                    out.push(Selectable::Pane { pos, win, pane });
                }
            }
        }
    }
    out
}

/// Interleave group section headers with entries and any expanded window
/// children, in filtered order.
fn build_display_rows(
    entries: &[NativeEntry],
    filtered: &[usize],
    expanded: &HashSet<String>,
) -> Vec<DisplayRow> {
    let mut rows = Vec::with_capacity(filtered.len() + 3);
    let mut last_group: Option<NativeGroup> = None;
    for (pos, entry_idx) in filtered.iter().enumerate() {
        let entry = &entries[*entry_idx];
        if last_group != Some(entry.group) {
            rows.push(DisplayRow::Header(entry.group.label()));
            last_group = Some(entry.group);
        }
        rows.push(DisplayRow::Entry(pos));
        if is_expanded(entry, expanded) {
            for (win, window) in entry.windows.iter().enumerate() {
                rows.push(DisplayRow::Window { pos, win });
                for pane in displayed_pane_indices(window, expanded) {
                    rows.push(DisplayRow::Pane { pos, win, pane });
                }
            }
        }
    }
    rows
}

/// Which way an expand/collapse key acts on the selected row.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TreeKey {
    Toggle,
    /// Toggle open/closed on a session row; a no-op on a window child. Lets
    /// `l`/→ both open a collapsed session and close an expanded one, without
    /// surprise-collapsing when pressed on a leaf window.
    EntryToggle,
    Collapse,
}

/// Apply an expand/collapse action to the row under `cursor`. Mutates the
/// `expanded` set (keyed by session name, so state survives list rebuilds) and
/// returns the possibly-adjusted cursor. Shared by the picker and the demo so
/// they behave identically.
pub(crate) fn apply_tree_key(
    key: TreeKey,
    selectables: &[Selectable],
    entries: &[NativeEntry],
    filtered: &[usize],
    expanded: &mut HashSet<String>,
    cursor: usize,
) -> usize {
    let Some(sel) = selectables.get(cursor) else {
        return cursor;
    };
    match sel {
        Selectable::Entry(pos) => {
            let Some(entry) = filtered.get(*pos).map(|idx| &entries[*idx]) else {
                return cursor;
            };
            let Some(session) = entry.session.as_ref().filter(|_| entry.is_expandable()) else {
                return cursor;
            };
            let open = expanded.contains(session);
            let should_open = match key {
                TreeKey::Collapse => false,
                TreeKey::Toggle | TreeKey::EntryToggle => !open,
            };
            if should_open {
                expanded.insert(session.clone());
            } else {
                collapse_session(entry, expanded);
            }
            cursor
        }
        Selectable::Window { pos, win } => {
            let Some(entry) = filtered.get(*pos).map(|idx| &entries[*idx]) else {
                return cursor;
            };
            let Some(window) = entry.windows.get(*win) else {
                return cursor;
            };
            let win_open = expanded.contains(&window.id);
            match key {
                // l/→ opens or closes this window's pane level; never the session.
                TreeKey::EntryToggle => {
                    if window.is_expandable() {
                        if win_open {
                            expanded.remove(&window.id);
                        } else {
                            expanded.insert(window.id.clone());
                        }
                    }
                    cursor
                }
                // Space toggles the pane level when there is one, else falls back
                // to collapsing the parent session (matching the old behaviour).
                TreeKey::Toggle if window.is_expandable() => {
                    if win_open {
                        expanded.remove(&window.id);
                    } else {
                        expanded.insert(window.id.clone());
                    }
                    cursor
                }
                // h/← closes an open pane level first, otherwise collapses the
                // parent session and moves the cursor back up to it.
                TreeKey::Collapse if win_open => {
                    expanded.remove(&window.id);
                    cursor
                }
                TreeKey::Collapse | TreeKey::Toggle => {
                    collapse_session(entry, expanded);
                    cursor_to_entry(selectables, *pos, cursor)
                }
            }
        }
        // On a pane leaf, collapse/toggle closes the parent window and moves the
        // cursor back up to it; expand (l/→) is a no-op.
        Selectable::Pane { pos, win, .. } => match key {
            TreeKey::EntryToggle => cursor,
            TreeKey::Collapse | TreeKey::Toggle => {
                if let Some(window) = filtered
                    .get(*pos)
                    .and_then(|idx| entries[*idx].windows.get(*win))
                {
                    expanded.remove(&window.id);
                }
                selectables
                    .iter()
                    .position(|s| matches!(s, Selectable::Window { pos: p, win: w } if p == pos && w == win))
                    .unwrap_or(cursor)
            }
        },
    }
}

/// Collapse a session: drop it and every one of its windows from the expanded
/// set, so a re-open starts clean and no stale window ids linger.
fn collapse_session(entry: &NativeEntry, expanded: &mut HashSet<String>) {
    if let Some(session) = entry.session.as_ref() {
        expanded.remove(session);
    }
    for window in &entry.windows {
        expanded.remove(&window.id);
    }
}

/// The cursor index of the `Entry` row at filtered position `pos`.
fn cursor_to_entry(selectables: &[Selectable], pos: usize, fallback: usize) -> usize {
    selectables
        .iter()
        .position(|s| matches!(s, Selectable::Entry(p) if *p == pos))
        .unwrap_or(fallback)
}

/// Whether any workspace row currently on screen has an agent — decides whether
/// to keep polling so the agent spinner animates. Shares the exact scroll math
/// used to render, so it never disagrees with what's visible.
pub(crate) fn visible_has_agent(
    entries: &[NativeEntry],
    filtered: &[usize],
    expanded: &HashSet<String>,
    cursor: usize,
) -> bool {
    let (_, rows) = terminal::size().unwrap_or((100, 30));
    let rows = rows as usize;
    let display = build_display_rows(entries, filtered, expanded);
    let cursor_disp = cursor_display_index(&display, cursor);
    let (body_start, body_end) = picker_body_range(rows);
    let body_rows = body_end.saturating_sub(body_start);
    let start = cursor_disp.saturating_sub(body_rows.saturating_sub(1));
    display
        .iter()
        .skip(start)
        .take(body_rows)
        .any(|row| match row {
            DisplayRow::Entry(pos) => entries[filtered[*pos]].tags.contains(&"agent"),
            DisplayRow::Pane { pos, win, pane } => entries[filtered[*pos]]
                .windows
                .get(*win)
                .and_then(|w| w.pane_rows.get(*pane))
                .is_some_and(|p| p.agent),
            _ => false,
        })
}

/// Display-row index of the `cursor`-th navigable row (skipping headers).
fn cursor_display_index(display: &[DisplayRow], cursor: usize) -> usize {
    let mut count = 0usize;
    for (idx, row) in display.iter().enumerate() {
        if matches!(row, DisplayRow::Header(_)) {
            continue;
        }
        if count == cursor {
            return idx;
        }
        count += 1;
    }
    display.len().saturating_sub(1)
}

/// The filtered entry position that a display row belongs to (its parent, for a
/// window child). Used to drive the preview pane.
fn display_row_entry_pos(row: &DisplayRow) -> Option<usize> {
    match row {
        DisplayRow::Entry(pos) | DisplayRow::Window { pos, .. } | DisplayRow::Pane { pos, .. } => {
            Some(*pos)
        }
        DisplayRow::Header(_) => None,
    }
}

/// Build a footer string of `key label` pairs with accent keycaps, padded to `cols`.
fn footer_keys(pairs: &[(&str, &str)], cols: usize, theme: &Theme) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    out.push_str(&raw_styled("  ", theme.footer));
    used += 2;
    for (key, label) in pairs {
        let cap = format!(" {key} ");
        let lab = format!("{label}  ");
        let seg_w = display_width(&cap) + display_width(&lab);
        if used + seg_w > cols {
            break;
        }
        out.push_str(&raw_styled(&cap, theme.key));
        out.push_str(&raw_styled(&lab, theme.footer));
        used += seg_w;
    }
    if used < cols {
        out.push_str(&raw_styled(&" ".repeat(cols - used), theme.footer));
    }
    out
}

/// Draw the status bar: accent brand + shown/total + scope (+ filter echo).
fn draw_status(
    frame: &mut [String],
    cols: usize,
    view: PickerView<'_>,
    shown: usize,
    total: usize,
    theme: &Theme,
) {
    let brand_text = format!("  {}", labels().brand);
    let brand_w = display_width(&brand_text).min(cols);
    set_frame_segment(&mut frame[0], 0, cols, "", theme.title);
    set_frame_segment(&mut frame[0], 0, brand_w, &brand_text, theme.brand);

    // Version tag in the dim status style right after the brand, sourced from the
    // crate version at build time so the menu always reflects the running binary.
    let version_text = format!(" v{}", env!("CARGO_PKG_VERSION"));
    let version_w = display_width(&version_text);
    if cols > brand_w {
        set_frame_segment(
            &mut frame[0],
            brand_w,
            (cols - brand_w).min(version_w),
            &version_text,
            theme.title,
        );
    }

    let query = view.filter.text();
    let filter_note = if view.filter.is_empty() {
        String::new()
    } else {
        format!("  {} {query}", GLYPHS.filter)
    };
    // Fleet summary (T4): waiting first — it's the count that most needs the user.
    let fleet = view.fleet;
    let fleet_note = if fleet.is_empty() {
        String::new()
    } else {
        let mut parts = Vec::new();
        if fleet.waiting > 0 {
            parts.push(format!("◆ {}", fleet.waiting));
        }
        if fleet.working > 0 {
            parts.push(format!("● {}", fleet.working));
        }
        if fleet.idle > 0 {
            parts.push(format!("○ {}", fleet.idle));
        }
        format!("  ·  {}", parts.join(" "))
    };
    // The filter echo (what the user is actively typing) comes before the fleet
    // counts, so on a narrow status bar the static counts truncate first and the
    // live filter text is preserved.
    let status = format!(
        "  {shown}/{total} · {}{filter_note}{fleet_note}",
        view.mode.label(),
    );
    let status_col = (brand_w + version_w).min(cols);
    if cols > status_col {
        set_frame_segment(
            &mut frame[0],
            status_col,
            cols - status_col,
            &status,
            theme.title,
        );
    }
}

/// Draw a group section header: dim label followed by a hairline rule.
fn draw_group_header(
    frame_line: &mut String,
    col: usize,
    width: usize,
    label: &str,
    theme: &Theme,
) {
    let text = format!("  {label} ");
    let text_w = display_width(&text).min(width);
    set_frame_segment(frame_line, col, width, "", theme.panel);
    set_frame_segment(frame_line, col, text_w, &text, theme.group);
    if width > text_w {
        set_frame_segment(
            frame_line,
            col + text_w,
            width - text_w,
            &"─".repeat(width - text_w),
            theme.divider,
        );
    }
}

/// Draw one workspace row: colored marker + glyph, then the aligned columns.
fn draw_entry_row(
    frame_line: &mut String,
    list_width: usize,
    entry: &NativeEntry,
    selected: bool,
    query: &str,
    theme: &Theme,
) {
    let row_style = if selected {
        theme.selected
    } else {
        theme.panel
    };
    // Fill the whole list area with the row background first.
    set_frame_segment(frame_line, 0, list_width, "", row_style);

    // Marker (accent bar on the selected row).
    let marker = if selected { GLYPHS.marker } else { " " };
    let marker_style = if selected { theme.marker } else { theme.panel };
    set_frame_segment(frame_line, 0, 1, marker, marker_style);

    // State glyph at column 2, colored by state (or bright when selected).
    let glyph = entry_glyph(entry).to_string();
    let glyph_col_style = if selected {
        theme.selected
    } else {
        glyph_style(entry, theme)
    };
    set_frame_segment(frame_line, 2, 1, &glyph, glyph_col_style);

    // Columns start after the fixed prefix, inset by a right gutter so the last
    // value never lands in the terminal's final (clip-prone) cell.
    if list_width > PREFIX + RIGHT_GUTTER {
        let content_width = list_width - PREFIX - RIGHT_GUTTER;
        let columns = entry_columns(entry, content_width);
        let highlighted = highlight_matches(
            &columns,
            query,
            content_width,
            row_style,
            theme.match_highlight,
        );
        set_frame_raw_segment(frame_line, PREFIX, &highlighted);
    }
}

/// Draw one expanded window child row: an indented tree connector under the
/// parent session, then window columns aligned beneath the parent's columns.
fn draw_window_row(
    frame_line: &mut String,
    list_width: usize,
    entry: &NativeEntry,
    win_idx: usize,
    selected: bool,
    query: &str,
    theme: &Theme,
) {
    let Some(win) = entry.windows.get(win_idx) else {
        return;
    };
    let is_last = win_idx + 1 == entry.windows.len();
    let row_style = if selected { theme.selected } else { theme.panel };
    set_frame_segment(frame_line, 0, list_width, "", row_style);

    // Accent marker on the selected row (matches entry rows).
    let marker = if selected { GLYPHS.marker } else { " " };
    let marker_style = if selected { theme.marker } else { theme.panel };
    set_frame_segment(frame_line, 0, 1, marker, marker_style);

    // Tree connector in the glyph gutter: `└`/`├` then `─`, so the window name
    // aligns exactly under the parent session name at PREFIX.
    let connector = if is_last { GLYPHS.tree_last } else { GLYPHS.tree_mid };
    let conn_style = if selected { theme.selected } else { theme.group };
    set_frame_segment(frame_line, 2, 2, connector, conn_style);

    if list_width > PREFIX + RIGHT_GUTTER {
        let content_width = list_width - PREFIX - RIGHT_GUTTER;
        let columns = window_columns(win, content_width);
        let highlighted = highlight_matches(
            &columns,
            query,
            content_width,
            row_style,
            theme.match_highlight,
        );
        set_frame_raw_segment(frame_line, PREFIX, &highlighted);
    }
}

/// Draw one pane leaf (third tree level): a deeper tree connector under the
/// window, then the pane's model/state columns.
#[allow(clippy::too_many_arguments)]
fn draw_pane_row(
    frame_line: &mut String,
    list_width: usize,
    window: &crate::native_state::WindowRow,
    pane_idx: usize,
    is_last: bool,
    selected: bool,
    query: &str,
    theme: &Theme,
) {
    let Some(pane) = window.pane_rows.get(pane_idx) else {
        return;
    };
    let row_style = if selected { theme.selected } else { theme.panel };
    set_frame_segment(frame_line, 0, list_width, "", row_style);

    let marker = if selected { GLYPHS.marker } else { " " };
    let marker_style = if selected { theme.marker } else { theme.panel };
    set_frame_segment(frame_line, 0, 1, marker, marker_style);

    // Connector inset one level deeper than a window child (cols 4-5), so panes
    // visibly nest under their window.
    let connector = if is_last { GLYPHS.tree_last } else { GLYPHS.tree_mid };
    let conn_style = if selected { theme.selected } else { theme.group };
    set_frame_segment(frame_line, 4, 2, connector, conn_style);

    const PANE_PREFIX: usize = 6;
    if list_width > PANE_PREFIX + RIGHT_GUTTER {
        let content_width = list_width - PANE_PREFIX - RIGHT_GUTTER;
        let columns = pane_columns(pane, content_width);
        let highlighted = highlight_matches(
            &columns,
            query,
            content_width,
            row_style,
            theme.match_highlight,
        );
        set_frame_raw_segment(frame_line, PANE_PREFIX, &highlighted);
    }
}

/// Draw the detail preview pane for the selected entry.
#[allow(clippy::too_many_arguments)]
fn draw_preview(
    frame: &mut [String],
    entry: Option<&NativeEntry>,
    body_start: usize,
    body_end: usize,
    preview_col: usize,
    preview_width: usize,
    rows: usize,
    theme: &Theme,
) {
    if body_start >= rows {
        return;
    }
    set_frame_segment(
        &mut frame[body_start],
        preview_col,
        preview_width,
        &format!("  {}", labels().details),
        theme.panel_header,
    );
    let Some(entry) = entry else { return };
    // Skip the leading "Workspace" label line; show the name as an accent title.
    let mut lines = entry.detail.lines().skip(1);
    let mut row = body_start + 2;
    if let Some(first) = lines.next() {
        if row < body_end.min(rows) {
            set_frame_segment(
                &mut frame[row],
                preview_col,
                preview_width,
                &format!("  {}", first.trim_start_matches("Name: ")),
                theme.detail_title,
            );
            row += 1;
        }
    }
    for line in lines {
        if row >= body_end.min(rows) {
            break;
        }
        set_frame_segment(
            &mut frame[row],
            preview_col,
            preview_width,
            &format!("  {line}"),
            theme.panel,
        );
        row += 1;
    }
}

pub(crate) fn draw_native_picker(
    entries: &[NativeEntry],
    filtered: &[usize],
    expanded: &HashSet<String>,
    cursor: usize,
    view: PickerView<'_>,
    theme: &Theme,
) -> Result<(), AppError> {
    let (cols, rows) = terminal::size().unwrap_or((100, 30));
    let cols = cols as usize;
    let rows = rows as usize;
    let mut frame = vec![blank_styled_line(cols, theme.panel); rows.max(1)];
    let query = view.filter.text();

    draw_status(&mut frame, cols, view, filtered.len(), entries.len(), theme);

    if view.show_help {
        draw_help(&mut frame, cols, rows, view.help_scroll, theme);
        return flush_frame(frame);
    }

    let compact = picker_uses_compact_height(rows);
    let layout = if compact {
        None
    } else {
        Some(picker_layout(cols))
    };
    let list_width = layout.map(|l| l.list_width).unwrap_or(cols);
    let (body_start, body_end) = picker_body_range(rows);
    let body_rows = body_end.saturating_sub(body_start);

    let display = build_display_rows(entries, filtered, expanded);
    let cursor_disp = cursor_display_index(&display, cursor);
    // Scroll so the cursor's display row stays visible (cursor drifts to bottom).
    let start = cursor_disp.saturating_sub(body_rows.saturating_sub(1));

    for visual in 0..body_rows {
        let frame_row = body_start + visual;
        if frame_row >= rows {
            break;
        }
        let disp_idx = start + visual;
        let Some(disp) = display.get(disp_idx) else {
            continue;
        };
        let selected = disp_idx == cursor_disp;
        match disp {
            DisplayRow::Header(label) => {
                draw_group_header(&mut frame[frame_row], 0, list_width, label, theme);
            }
            DisplayRow::Entry(pos) => {
                let entry = &entries[filtered[*pos]];
                draw_entry_row(&mut frame[frame_row], list_width, entry, selected, query, theme);
            }
            DisplayRow::Window { pos, win } => {
                let entry = &entries[filtered[*pos]];
                draw_window_row(
                    &mut frame[frame_row],
                    list_width,
                    entry,
                    *win,
                    selected,
                    query,
                    theme,
                );
            }
            DisplayRow::Pane { pos, win, pane } => {
                let entry = &entries[filtered[*pos]];
                if let Some(window) = entry.windows.get(*win) {
                    // The tree connector's `└` must mark the last *shown* pane,
                    // which is a subset when only agent panes are auto-revealed.
                    let is_last =
                        displayed_pane_indices(window, expanded).last() == Some(pane);
                    draw_pane_row(
                        &mut frame[frame_row],
                        list_width,
                        window,
                        *pane,
                        is_last,
                        selected,
                        query,
                        theme,
                    );
                }
            }
        }
    }

    // Preview pane + divider on wide layouts. A selected window still previews
    // its parent session (whose detail lists every window).
    if let Some(layout) = layout {
        if layout.preview {
            for frame_row in frame.iter_mut().take(body_end.min(rows)).skip(body_start) {
                set_frame_segment(frame_row, layout.divider_col, 1, GLYPHS.divider, theme.divider);
            }
            let selected = display
                .get(cursor_disp)
                .and_then(display_row_entry_pos)
                .and_then(|pos| filtered.get(pos))
                .map(|idx| &entries[*idx]);
            draw_preview(
                &mut frame,
                selected,
                body_start,
                body_end,
                layout.preview_col,
                layout.preview_width,
                rows,
                theme,
            );
        }
    }

    draw_footer(&mut frame, cols, rows, view, theme);
    flush_frame(frame)
}

/// Body rows available to the help view: everything below the status bar (row 0)
/// and the "help" title (row 1). Shared by the renderer and the scroll clamp so
/// they never disagree about how far the body can scroll.
pub(crate) fn help_visible_rows(rows: usize) -> usize {
    rows.saturating_sub(2)
}

/// The furthest the help view can scroll for a terminal of `rows` height, so the
/// last line still lands on-screen and `j` stops at the bottom.
pub(crate) fn help_max_scroll(rows: usize) -> usize {
    native_help_body()
        .len()
        .saturating_sub(help_visible_rows(rows))
}

fn draw_help(frame: &mut [String], cols: usize, rows: usize, scroll: usize, theme: &Theme) {
    let body = native_help_body();
    let avail = help_visible_rows(rows);
    let scroll = scroll.min(body.len().saturating_sub(avail));
    // Title doubles as a scroll indicator when there's more above/below.
    let more_above = scroll > 0;
    let more_below = scroll + avail < body.len();
    let help = labels().help;
    let title = match (more_above, more_below) {
        (true, true) => format!("{help} ↕"),
        (false, true) => format!("{help} ↓"),
        (true, false) => format!("{help} ↑"),
        (false, false) => help.to_string(),
    };
    if rows > 1 {
        set_frame_segment(
            &mut frame[1],
            0,
            cols,
            &section_title(&title, cols),
            theme.panel_header,
        );
    }
    for (idx, line) in body.iter().skip(scroll).take(avail).enumerate() {
        let row = idx + 2;
        if row < rows {
            set_frame_segment(&mut frame[row], 0, cols, line, theme.panel);
        }
    }
}

fn draw_footer(
    frame: &mut [String],
    cols: usize,
    rows: usize,
    view: PickerView<'_>,
    theme: &Theme,
) {
    if rows == 0 {
        return;
    }
    let last = rows - 1;
    if view.edit_filter {
        let footer = format!(
            "  {}: {}   ⏎ {}   Esc normal   C-w word   C-u clear",
            labels().filter_prompt,
            view.filter.display_with_cursor(),
            labels().action_open,
        );
        set_frame_segment(&mut frame[last], 0, cols, &footer, theme.filter_active);
        return;
    }
    // A stable `Sessions · Agents · Layouts · Dirs` switcher plus the actions for
    // the current mode. footer_keys truncates from the right on narrow terminals.
    let l = labels();
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    pairs.push(match view.mode {
        PickerMode::Dirs | PickerMode::Layouts => ("⏎", l.action_start),
        _ => ("⏎", l.action_open),
    });
    for m in PickerMode::ALL {
        pairs.push((m.key_label(), m.label()));
    }
    if view.mode == PickerMode::Sessions {
        pairs.push(("l", l.action_tree));
    }
    pairs.push(("/", l.action_filter));
    pairs.push(("?", l.action_help));
    pairs.push(("q", l.action_close));
    let footer = footer_keys(&pairs, cols, theme);
    set_frame_raw_segment(&mut frame[last], 0, &footer);
}

fn flush_frame(frame: Vec<String>) -> Result<(), AppError> {
    let mut out = String::from("\x1b[H");
    out.push_str(&frame.join("\r\n"));
    let mut stdout = std::io::stdout();
    stdout.write_all(out.as_bytes()).map_err(|e| {
        AppError::new(ErrorCode::ProviderFailure, "failed to write picker frame").with_source(e)
    })?;
    stdout.flush().map_err(|e| {
        AppError::new(ErrorCode::ProviderFailure, "failed to flush picker").with_source(e)
    })
}
