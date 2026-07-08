use std::time::SystemTime;

use super::columns::{display_width, fit, truncate_ellipsis, Align};
// `display_width` is re-exported from `columns` via `ui::*`; text helpers below
// delegate to `columns` so every width/truncate/pad calculation has one home.

pub(crate) fn truncate_to_width(s: &str, width: usize) -> String {
    truncate_ellipsis(s, width)
}

pub(crate) fn pad_to_width(s: &str, width: usize) -> String {
    fit(s, width, Align::Left)
}

pub(crate) fn styled_segment(width: usize, text: &str, ansi_style: &str) -> String {
    format!("{ansi_style}{}\x1b[0m", pad_to_width(text, width))
}

pub(crate) fn raw_styled(text: &str, ansi_style: &str) -> String {
    format!("{ansi_style}{text}\x1b[0m")
}

pub(crate) fn blank_styled_line(width: usize, panel_style: &str) -> String {
    styled_segment(width, "", panel_style)
}

pub(crate) fn set_frame_segment(
    line: &mut String,
    col: usize,
    width: usize,
    text: &str,
    ansi_style: &str,
) {
    if width == 0 {
        return;
    }
    line.push_str(&format!("\x1b[{}G", col + 1));
    line.push_str(&styled_segment(width, text, ansi_style));
}

pub(crate) fn set_frame_raw_segment(line: &mut String, col: usize, text: &str) {
    line.push_str(&format!("\x1b[{}G{text}", col + 1));
}

pub(crate) fn hline(width: usize) -> String {
    "─".repeat(width)
}

pub(crate) fn section_title(label: &str, width: usize) -> String {
    if width <= 4 {
        return hline(width);
    }
    let title = format!("┤ {label} ├");
    let title_width = display_width(&title);
    if title_width >= width {
        return truncate_to_width(label, width);
    }
    let right = width - title_width;
    format!("{title}{}", "─".repeat(right))
}

pub(crate) fn spinner_frame() -> &'static str {
    const FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    let millis = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    FRAMES[((millis / 160) as usize) % FRAMES.len()]
}
