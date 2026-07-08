#[derive(Debug, Clone, Copy)]
pub(crate) struct PickerLayout {
    pub(crate) preview: bool,
    pub(crate) list_width: usize,
    pub(crate) divider_col: usize,
    pub(crate) preview_col: usize,
    pub(crate) preview_width: usize,
}

pub(crate) fn picker_layout(cols: usize) -> PickerLayout {
    let preview = cols >= 118;
    if !preview {
        return PickerLayout {
            preview,
            list_width: cols,
            divider_col: cols,
            preview_col: cols,
            preview_width: 0,
        };
    }

    let min_preview = 36usize;
    let max_preview = 58usize;
    let preview_width = (cols / 3).clamp(min_preview, max_preview);
    let preview_col = cols.saturating_sub(preview_width);
    let divider_col = preview_col.saturating_sub(2);
    let list_width = divider_col.saturating_sub(1);
    PickerLayout {
        preview,
        list_width,
        divider_col,
        preview_col,
        preview_width,
    }
}

pub(crate) fn picker_uses_compact_height(rows: usize) -> bool {
    rows <= 10
}

pub(crate) fn picker_body_range(rows: usize) -> (usize, usize) {
    if picker_uses_compact_height(rows) {
        // compact: row 0 is the status bar, last row is the footer.
        (1, rows.saturating_sub(1))
    } else {
        // normal: row 0 status bar, row 1 blank, last row footer.
        (2, rows.saturating_sub(1))
    }
}

pub(crate) fn picker_body_rows(rows: usize) -> usize {
    let (start, end) = picker_body_range(rows);
    end.saturating_sub(start)
}
