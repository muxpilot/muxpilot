use std::collections::HashSet;
use std::io::Write;

use crate::error::{AppError, ErrorCode};
use crossterm::terminal;

use crate::native_state::{native_help_body, FilterInput, NativeEntry, NativeGroup, SearchMode};
use crate::ui::*;

/// Fixed left gutter before the column area: `marker + space + glyph + space`.
const PREFIX: usize = 4;
/// Trailing columns kept clear on the right so the rightmost value never sits in
/// the terminal's final cell — the row background still fills edge-to-edge, but
/// text is inset. Keeps last-activity readable and avoids terminal/recorder
/// edge-cell clipping.
const RIGHT_GUTTER: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PickerScreen {
    Main,
    Directories,
}

impl PickerScreen {
    fn label(self) -> &'static str {
        match self {
            Self::Main => "workspaces",
            Self::Directories => "directories",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PickerView<'a> {
    pub(crate) filter: &'a FilterInput,
    pub(crate) mode: SearchMode,
    pub(crate) screen: PickerScreen,
    pub(crate) show_help: bool,
    pub(crate) edit_filter: bool,
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
}

/// A navigable row — everything the cursor can land on (headers excluded). The
/// cursor is an index into the ordered list this produces.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Selectable {
    Entry(usize),
    Window { pos: usize, win: usize },
}

/// Whether the entry at filtered position `pos` is currently expanded.
fn is_expanded(entry: &NativeEntry, expanded: &HashSet<String>) -> bool {
    entry.is_expandable()
        && entry
            .session
            .as_ref()
            .is_some_and(|s| expanded.contains(s))
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
            for win in 0..entry.windows.len() {
                out.push(Selectable::Window { pos, win });
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
            for win in 0..entry.windows.len() {
                rows.push(DisplayRow::Window { pos, win });
            }
        }
    }
    rows
}

/// Which way an expand/collapse key acts on the selected row.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TreeKey {
    Toggle,
    Expand,
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
                TreeKey::Expand => true,
                TreeKey::Collapse => false,
                TreeKey::Toggle => !open,
            };
            if should_open {
                expanded.insert(session.clone());
            } else {
                expanded.remove(session);
            }
            cursor
        }
        // On a window child, collapse/toggle closes the parent and moves the
        // cursor back up to it; expand is a no-op.
        Selectable::Window { pos, .. } => match key {
            TreeKey::Expand => cursor,
            TreeKey::Collapse | TreeKey::Toggle => {
                if let Some(session) = filtered.get(*pos).and_then(|idx| entries[*idx].session.as_ref())
                {
                    expanded.remove(session);
                }
                selectables
                    .iter()
                    .position(|s| matches!(s, Selectable::Entry(p) if p == pos))
                    .unwrap_or(cursor)
            }
        },
    }
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
        DisplayRow::Entry(pos) | DisplayRow::Window { pos, .. } => Some(*pos),
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
    let brand = "  muxpilot";
    let brand_w = display_width(brand).min(cols);
    set_frame_segment(&mut frame[0], 0, cols, "", theme.title);
    set_frame_segment(&mut frame[0], 0, brand_w, brand, theme.brand);

    let query = view.filter.text();
    let filter_note = if view.filter.is_empty() {
        String::new()
    } else {
        format!("  ⌕ {query}")
    };
    let status = format!(
        "  {shown}/{total} · {} · {}{filter_note}",
        view.screen.label(),
        view.mode.label(),
    );
    let status_col = brand_w;
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
    let marker = if selected { "▍" } else { " " };
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
    let marker = if selected { "▍" } else { " " };
    let marker_style = if selected { theme.marker } else { theme.panel };
    set_frame_segment(frame_line, 0, 1, marker, marker_style);

    // Tree connector in the glyph gutter: `└`/`├` then `─`, so the window name
    // aligns exactly under the parent session name at PREFIX.
    let connector = if is_last { "└─" } else { "├─" };
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
        "  details",
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
        draw_help(&mut frame, cols, rows, theme);
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
        }
    }

    // Preview pane + divider on wide layouts. A selected window still previews
    // its parent session (whose detail lists every window).
    if let Some(layout) = layout {
        if layout.preview {
            for frame_row in frame.iter_mut().take(body_end.min(rows)).skip(body_start) {
                set_frame_segment(frame_row, layout.divider_col, 1, "│", theme.divider);
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

fn draw_help(frame: &mut [String], cols: usize, rows: usize, theme: &Theme) {
    if rows > 1 {
        set_frame_segment(
            &mut frame[1],
            0,
            cols,
            &section_title("help", cols),
            theme.panel_header,
        );
    }
    for (idx, line) in native_help_body()
        .into_iter()
        .take(rows.saturating_sub(3))
        .enumerate()
    {
        let row = idx + 2;
        if row < rows {
            set_frame_segment(&mut frame[row], 0, cols, &line, theme.panel);
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
            "  FILTER: {}   ⏎ open   Esc normal   C-w word   C-u clear",
            view.filter.display_with_cursor()
        );
        set_frame_segment(&mut frame[last], 0, cols, &footer, theme.filter_active);
        return;
    }
    let pairs: &[(&str, &str)] = match view.screen {
        PickerScreen::Main => &[
            ("⏎", "open"),
            ("l", "tree"),
            ("/", "filter"),
            ("d", "dirs"),
            ("⇥", "scope"),
            ("?", "help"),
            ("q", "close"),
        ],
        PickerScreen::Directories => &[
            ("⏎", "start"),
            ("/", "filter"),
            ("Esc", "back"),
            ("?", "help"),
            ("q", "close"),
        ],
    };
    let footer = footer_keys(pairs, cols, theme);
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
