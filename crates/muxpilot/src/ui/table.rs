use crate::native_state::{NativeEntry, NativeGroup, PaneRow, SearchMode, Theme, WindowRow};

use super::columns::{render_row, solve, Align, Constraint};
use super::text::{pad_to_width, raw_styled, styled_segment};

/// Split a pre-rendered entry line into its logical columns.
///
/// The line is built as `"{glyph} {name} · {caps} · {status} · {last}"`; this
/// is the single place that parsing lives.
fn split_entry_columns(entry: &NativeEntry) -> (&str, &str, &str, &str) {
    let mut parts = entry.line.split(" · ");
    let name = parts.next().unwrap_or(&entry.line);
    let caps = parts.next().unwrap_or("");
    let activity = parts.next().unwrap_or("");
    let last = parts.next().unwrap_or("");
    (name, caps, activity, last)
}

/// The leading state glyph (`◆`/`●`/`○`), drawn separately so it can be colored.
pub(crate) fn entry_glyph(entry: &NativeEntry) -> char {
    entry.line.chars().next().unwrap_or(' ')
}

/// The workspace name with its leading glyph stripped. Every line is built as
/// `"<glyph> <name> · …"`, so dropping the first whitespace-delimited token
/// removes whatever state glyph leads it (◆/●/○/◇/×/·/…) — the glyph is drawn
/// separately in the gutter, so it must not appear again in the name column.
fn entry_name(entry: &NativeEntry) -> &str {
    let (name, _, _, _) = split_entry_columns(entry);
    name.split_once(' ')
        .map(|(_, rest)| rest)
        .unwrap_or(name)
        .trim_start()
}

pub(crate) fn entry_sort_name(entry: &NativeEntry) -> String {
    entry_name(entry).to_ascii_lowercase()
}

/// Column plan for the list body at a given content `width` (already excluding
/// the fixed glyph/marker gutter). Returns the cells, their solved widths, and
/// alignments — header and body rows share this so they can never drift.
fn columns_for(width: usize, name: &str, caps: &str, status: &str, last: &str) -> String {
    // Choose which meta columns fit, widest-screen-first. Every width is solved
    // through `columns::solve`, so the rendered row is always exactly `width`.
    let (caps_w, status_w, last_w) = if width >= 60 {
        (7usize, 10usize, 4usize)
    } else if width >= 46 {
        (6, 8, 4)
    } else if width >= 32 {
        (0, 7, 4)
    } else {
        (0, 0, 4)
    };

    let mut cells: Vec<&str> = vec![name];
    let mut cons: Vec<Constraint> = vec![Constraint::Min(6)];
    let mut aligns: Vec<Align> = vec![Align::Left];
    if caps_w > 0 {
        cells.push(caps);
        cons.push(Constraint::Fixed(caps_w));
        aligns.push(Align::Left);
    }
    if status_w > 0 {
        cells.push(status);
        cons.push(Constraint::Fixed(status_w));
        aligns.push(Align::Left);
    }
    cells.push(last);
    cons.push(Constraint::Fixed(last_w));
    aligns.push(Align::Right);

    let widths = solve(width, 1, &cons);
    render_row(&cells, &widths, &aligns, 1)
}

/// Column header built from the same plan as body rows (used in tests to assert
/// header/body alignment; handy for a future opt-in header row).
#[allow(dead_code)]
pub(crate) fn entry_header(width: usize) -> String {
    columns_for(width, "workspace", "caps", "status", "last")
}

pub(crate) fn entry_columns(entry: &NativeEntry, width: usize) -> String {
    let (_, caps, status, last) = split_entry_columns(entry);
    columns_for(width, entry_name(entry), caps, status, last)
}

/// Column layout for an expanded window child row. Built through the same
/// `columns_for` plan as the parent entry (same width, same solver), so the
/// window's meta columns line up exactly under the session's — never drifting.
/// The `◍` keeps a trailing space before its count for the same overlap reason
/// as `capability_icons`.
pub(crate) fn window_columns(win: &WindowRow, width: usize) -> String {
    let mut caps = format!("{}p", win.panes);
    if win.agents > 0 {
        caps.push_str(&format!(" ◍ {}", win.agents));
    }
    // A one-agent window surfaces that agent's state inline (glyph + short
    // label); multi-agent windows expand to show each pane's own state instead.
    let status = match win.inline_agent_status() {
        Some(inline) => inline.to_string(),
        None if win.active => "active".to_string(),
        None => String::new(),
    };
    columns_for(width, &win.name, &caps, &status, &win.activity)
}

/// Column layout for a pane leaf: the agent kind + model (or command) as the
/// name, its state, and its activity. Built through the same `columns_for` plan
/// so it aligns with the rows above it.
pub(crate) fn pane_columns(pane: &PaneRow, width: usize) -> String {
    columns_for(width, &pane.label, "", &pane.status, &pane.activity)
}

pub(crate) fn entry_style(entry: &NativeEntry, theme: &Theme) -> &'static str {
    match entry.group {
        NativeGroup::Running if entry.tags.contains(&"agent") => theme.agent,
        NativeGroup::Running => theme.active,
        NativeGroup::Configured | NativeGroup::Directories => theme.ready,
        // Agents-mode buckets: attention pulls the eye (accent), working is live
        // (green), quiet recedes (dim).
        NativeGroup::AgentNeedsYou => theme.current,
        NativeGroup::AgentWorking => theme.active,
        NativeGroup::AgentQuiet => theme.ready,
    }
}

/// Color for the leading state glyph, keyed off the glyph itself so `◆` (the
/// current workspace) always reads as the accent regardless of group.
pub(crate) fn glyph_style(entry: &NativeEntry, theme: &Theme) -> &'static str {
    match entry_glyph(entry) {
        '◆' | '＋' => theme.current,
        _ => entry_style(entry, theme),
    }
}

pub(crate) fn highlight_matches(
    text: &str,
    query: &str,
    width: usize,
    base_style: &str,
    match_style: &str,
) -> String {
    let padded = pad_to_width(text, width);
    let tokens: Vec<String> = query
        .split_whitespace()
        .map(|s| s.to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if tokens.is_empty() {
        return styled_segment(width, &padded, base_style);
    }

    let lower = padded.to_ascii_lowercase();
    let mut ranges: Vec<(usize, usize)> = tokens
        .iter()
        .filter_map(|token| lower.find(token).map(|start| (start, start + token.len())))
        .collect();
    ranges.sort_unstable();

    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        if let Some((_, last_end)) = merged.last_mut() {
            if start <= *last_end {
                *last_end = (*last_end).max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    if merged.is_empty() {
        return styled_segment(width, &padded, base_style);
    }

    let mut out = String::new();
    let mut byte = 0usize;
    for (start, end) in merged {
        if start > byte {
            out.push_str(&raw_styled(&padded[byte..start], base_style));
        }
        out.push_str(&raw_styled(&padded[start..end], match_style));
        byte = end;
    }
    if byte < padded.len() {
        out.push_str(&raw_styled(&padded[byte..], base_style));
    }
    out
}

pub(crate) fn entry_matches(entry: &NativeEntry, query: &str, mode: SearchMode) -> bool {
    if !mode.accepts(entry) {
        return false;
    }
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return true;
    }
    query
        .split_whitespace()
        .all(|token| entry.search_text.contains(token))
}
