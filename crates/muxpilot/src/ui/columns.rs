//! Encapsulated column layout + width math.
//!
//! Every display-width calculation, truncation, and padding used by the picker
//! flows through this module so columns can never drift out of alignment.
//!
//! The invariant this module guarantees: [`render_row`] always returns a string
//! whose display width equals `sum(widths) + gap * (n - 1)` — the same total the
//! caller planned with [`solve`]. Header rows and body rows built from the same
//! plan therefore line up column-for-column, regardless of the text inside a
//! cell (wide glyphs, long names, empty strings all pad/truncate deterministically).

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Display width of a string in terminal cells.
///
/// Single source of truth — do not call `unicode_width` directly elsewhere.
/// Control characters and unassigned widths count as zero so a stray byte can
/// never silently widen a cell past its budget.
pub(crate) fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

fn char_width(c: char) -> usize {
    UnicodeWidthChar::width(c).unwrap_or(0)
}

/// Truncate to at most `max` display cells, appending `…` when clipped.
///
/// The ellipsis itself occupies one cell, so the result never exceeds `max`.
pub(crate) fn truncate_ellipsis(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if display_width(s) <= max {
        return s.to_string();
    }
    // Reserve one cell for the ellipsis.
    let budget = max - 1;
    let mut out = String::new();
    let mut used = 0usize;
    for c in s.chars() {
        let w = char_width(c);
        if used + w > budget {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Align {
    Left,
    Right,
}

/// Truncate `s` to `width` then pad (with spaces) so the result is exactly
/// `width` display cells wide. Always returns a `width`-wide string.
pub(crate) fn fit(s: &str, width: usize, align: Align) -> String {
    let clipped = truncate_ellipsis(s, width);
    let used = display_width(&clipped);
    let pad = width - used; // safe: truncate_ellipsis guarantees used <= width
    match align {
        Align::Left => format!("{clipped}{}", " ".repeat(pad)),
        Align::Right => format!("{}{clipped}", " ".repeat(pad)),
    }
}

/// How a column claims horizontal space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // `Flex` is part of the layout API, exercised by tests.
pub(crate) enum Constraint {
    /// Exactly `n` cells (clamped down if space is scarce).
    Fixed(usize),
    /// A floor of `n` cells, then grows with any leftover space (weight 1).
    Min(usize),
    /// No floor; absorbs leftover space by `weight` relative to other flexes.
    Flex(u16),
}

/// Solve column widths for `total` cells with `gap` cells between each column.
///
/// Guarantees `sum(result) + gap * (n - 1) == total` whenever
/// `total >= gap * (n - 1)` (otherwise it returns all-zero widths). Fixed and
/// `Min` floors are honoured first; leftover space is distributed to flexible
/// columns by weight; any rounding remainder lands on the last flexible column
/// (or the last column if none are flexible) so the row always fills exactly.
pub(crate) fn solve(total: usize, gap: usize, cols: &[Constraint]) -> Vec<usize> {
    let n = cols.len();
    if n == 0 {
        return Vec::new();
    }
    let gaps = gap.saturating_mul(n - 1);
    if total <= gaps {
        return vec![0; n];
    }
    let avail = total - gaps;

    // Baseline demand: Fixed wants its size, Min wants its floor, Flex wants 0.
    let base: Vec<usize> = cols
        .iter()
        .map(|c| match c {
            Constraint::Fixed(w) => *w,
            Constraint::Min(w) => *w,
            Constraint::Flex(_) => 0,
        })
        .collect();
    let base_sum: usize = base.iter().sum();

    let mut widths = base.clone();

    if base_sum >= avail {
        // Not enough room even for the baseline: shrink from the last column
        // backward until we fit. Fixed/Min both yield here so nothing overflows.
        let mut over = base_sum - avail;
        for w in widths.iter_mut().rev() {
            if over == 0 {
                break;
            }
            let take = (*w).min(over);
            *w -= take;
            over -= take;
        }
        return widths;
    }

    // Distribute the surplus to flexible columns (Flex by weight, Min as weight 1).
    let mut surplus = avail - base_sum;
    let weights: Vec<u32> = cols
        .iter()
        .map(|c| match c {
            Constraint::Flex(w) => (*w).max(1) as u32,
            Constraint::Min(_) => 1,
            Constraint::Fixed(_) => 0,
        })
        .collect();
    let weight_sum: u32 = weights.iter().sum();

    if weight_sum == 0 {
        // No flexible columns; hand the surplus to the last column so we still
        // fill `total` exactly.
        *widths.last_mut().expect("n > 0") += surplus;
        return widths;
    }

    let mut last_flex = 0usize;
    for (i, w) in weights.iter().enumerate() {
        if *w > 0 {
            let share = (surplus as u64 * *w as u64 / weight_sum as u64) as usize;
            widths[i] += share;
            last_flex = i;
        }
    }
    // Assign the rounding remainder to the last flexible column.
    let assigned: usize = widths.iter().sum::<usize>() - base_sum;
    surplus -= assigned;
    widths[last_flex] += surplus;
    widths
}

/// Render one row: fit each cell to its planned width and join with `gap` spaces.
///
/// `cells`, `widths`, and `aligns` must be the same length. The returned string
/// is exactly `sum(widths) + gap * (n - 1)` cells wide.
pub(crate) fn render_row(cells: &[&str], widths: &[usize], aligns: &[Align], gap: usize) -> String {
    debug_assert_eq!(cells.len(), widths.len());
    debug_assert_eq!(cells.len(), aligns.len());
    let joiner = " ".repeat(gap);
    cells
        .iter()
        .zip(widths.iter())
        .zip(aligns.iter())
        .map(|((cell, width), align)| fit(cell, *width, *align))
        .collect::<Vec<_>>()
        .join(&joiner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_of_wide_and_narrow() {
        assert_eq!(display_width("abc"), 3);
        assert_eq!(display_width("●○◆"), 3); // symbol glyphs are single-width
        assert_eq!(display_width("日本"), 4); // CJK are double-width
    }

    #[test]
    fn truncate_adds_ellipsis_within_budget() {
        assert_eq!(truncate_ellipsis("hello", 10), "hello");
        assert_eq!(truncate_ellipsis("hello", 5), "hello");
        let t = truncate_ellipsis("hello world", 5);
        assert_eq!(t, "hell…");
        assert_eq!(display_width(&t), 5);
        assert_eq!(truncate_ellipsis("anything", 0), "");
    }

    #[test]
    fn truncate_never_splits_wide_glyph_over_budget() {
        // "日本語" is 6 cells; fitting to 5 must drop a whole glyph + ellipsis.
        let t = truncate_ellipsis("日本語", 5);
        assert!(display_width(&t) <= 5, "got width {}", display_width(&t));
    }

    #[test]
    fn fit_is_exact_width() {
        for (s, w) in [
            ("hi", 8),
            ("a very long label indeed", 8),
            ("", 5),
            ("日本", 3),
        ] {
            assert_eq!(display_width(&fit(s, w, Align::Left)), w, "left {s:?} {w}");
            assert_eq!(
                display_width(&fit(s, w, Align::Right)),
                w,
                "right {s:?} {w}"
            );
        }
    }

    #[test]
    fn solve_fills_total_exactly() {
        let cols = [
            Constraint::Min(10),
            Constraint::Fixed(6),
            Constraint::Fixed(8),
            Constraint::Fixed(5),
        ];
        for total in [20usize, 40, 80, 120, 200] {
            let gap = 1;
            let widths = solve(total, gap, &cols);
            let sum: usize = widths.iter().sum();
            let expected = sum + gap * (cols.len() - 1);
            if total >= gap * (cols.len() - 1) {
                assert_eq!(expected, total, "total={total} widths={widths:?}");
            }
        }
    }

    #[test]
    fn solve_degrades_without_overflow_when_cramped() {
        let cols = [Constraint::Fixed(20), Constraint::Fixed(20)];
        let widths = solve(10, 1, &cols);
        let sum: usize = widths.iter().sum();
        assert!(sum < 10, "must not overflow: {widths:?}");
    }

    #[test]
    fn flex_splits_by_weight() {
        let cols = [Constraint::Flex(1), Constraint::Flex(3)];
        let widths = solve(41, 1, &cols); // 40 usable
        assert_eq!(widths.iter().sum::<usize>(), 40);
        assert_eq!(widths[0], 10);
        assert_eq!(widths[1], 30);
    }

    #[test]
    fn render_row_is_planned_width_regardless_of_content() {
        let cols = [
            Constraint::Min(6),
            Constraint::Fixed(6),
            Constraint::Fixed(6),
        ];
        let aligns = [Align::Left, Align::Left, Align::Right];
        let gap = 2;
        for total in [24usize, 40, 60] {
            let widths = solve(total, gap, &cols);
            for cells in [
                ["short", "3w", "now"],
                [
                    "an extremely long workspace name that overflows",
                    "◍3",
                    "21h",
                ],
                ["", "", ""],
                ["日本語ワークスペース", "◍12", "5d"],
            ] {
                let refs: Vec<&str> = cells.to_vec();
                let row = render_row(&refs, &widths, &aligns, gap);
                assert_eq!(
                    display_width(&row),
                    total,
                    "total={total} row={row:?} widths={widths:?}"
                );
            }
        }
    }
}
