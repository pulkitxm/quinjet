use super::{HELP_ROWS, HelpRow};

pub(crate) fn help_shortcut_count() -> usize {
    HELP_ROWS
        .iter()
        .filter(|row| matches!(row, HelpRow::Shortcut { .. }))
        .count()
}

pub(crate) fn help_display_index(selected: usize) -> usize {
    HELP_ROWS
        .iter()
        .enumerate()
        .filter_map(|(index, row)| matches!(row, HelpRow::Shortcut { .. }).then_some(index))
        .nth(selected)
        .unwrap_or(0)
}

pub(crate) fn help_shortcut_index_at(display: usize) -> Option<usize> {
    if !matches!(HELP_ROWS.get(display), Some(HelpRow::Shortcut { .. })) {
        return None;
    }
    Some(
        HELP_ROWS
            .iter()
            .take(display)
            .filter(|row| matches!(row, HelpRow::Shortcut { .. }))
            .count(),
    )
}
