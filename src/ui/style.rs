#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;
pub(super) fn file_icon_span(path: &Path, theme: &Theme) -> Span<'static> {
    let icon = file_icons::for_path(path);
    Span::styled(icon.glyph, Style::default().fg(theme.syntax(icon.color)))
}

pub(super) const fn disclosure_glyph(expanded: bool) -> &'static str {
    if expanded { "⌄" } else { "›" }
}

pub(super) const fn disclosure_prefix(expanded: bool) -> &'static str {
    if expanded { " ⌄ " } else { " › " }
}

pub(super) fn set_text_cursor(
    frame: &mut Frame<'_>,
    area: Rect,
    input: &crate::app::TextBuffer,
    multiline: bool,
) {
    let before = input.before_cursor();
    let (row, column) = if multiline {
        let row = before
            .chars()
            .filter(|character| *character == '\n')
            .count();
        let column = before.rsplit('\n').next().unwrap_or_default().width();
        (row, column)
    } else {
        (0, before.replace('\n', " ").width())
    };
    let x = area
        .x
        .saturating_add(cells(column.min(area.width.saturating_sub(1) as usize)));
    let y = area
        .y
        .saturating_add(cells(row.min(area.height.saturating_sub(1) as usize)));
    frame.set_cursor_position((x, y));
}

pub(super) fn panel_block(title: String, focused: bool, theme: &Theme) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(if focused {
            theme.border_focus
        } else {
            theme.border
        }))
        .style(Style::default().bg(theme.panel).fg(theme.text))
}

pub(super) fn modal_block(title: &str, theme: &Theme) -> Block<'static> {
    Block::default()
        .title(title.to_owned())
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_focus))
        .style(Style::default().bg(theme.panel).fg(theme.text))
}

pub(super) const fn status_color(status: ChangeStatus, theme: &Theme) -> Color {
    match status {
        ChangeStatus::Added | ChangeStatus::Untracked => theme.added,
        ChangeStatus::Deleted => theme.removed,
        ChangeStatus::Conflicted => theme.conflict,
        ChangeStatus::Modified
        | ChangeStatus::Renamed
        | ChangeStatus::Copied
        | ChangeStatus::TypeChanged => theme.modified,
    }
}

pub(super) const fn graph_color(index: usize, theme: &Theme) -> Color {
    match index % 4 {
        0 => theme.accent,
        1 => theme.modified,
        2 => theme.added,
        _ => theme.conflict,
    }
}

pub(super) const fn history_glyph(
    commit: &crate::git::history::Commit,
    index: usize,
) -> &'static str {
    if commit.parent_ids.len() > 1 {
        "●╮ "
    } else if index > 0 {
        "●│ "
    } else {
        "●  "
    }
}

pub(super) fn clean_decoration(decoration: &str) -> &str {
    decoration.strip_prefix("HEAD -> ").unwrap_or(decoration)
}

pub(super) fn ensure_offset(offset: &mut usize, cursor: usize, height: usize, length: usize) {
    if height == 0 || length == 0 {
        *offset = 0;
        return;
    }
    if cursor < *offset {
        *offset = cursor;
    } else if cursor >= *offset + height {
        *offset = cursor + 1 - height;
    }
    *offset = (*offset).min(length.saturating_sub(height));
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
pub(super) fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let width = width.min(outer.width);
    let height = height.min(outer.height);
    Rect::new(
        outer.x + (outer.width - width) / 2,
        outer.y + (outer.height - height) / 2,
        width,
        height,
    )
}

pub(super) fn truncate_end(value: &str, width: usize) -> String {
    if value.width() <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_owned();
    }
    format!("{}…", slice_width(value, 0, width - 1))
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
pub(super) fn truncate_middle(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if value.width() <= width {
        return value.to_owned();
    }
    if width <= 1 {
        return "…".to_owned();
    }
    let left_width = (width - 1) * 2 / 3;
    let right_width = width - 1 - left_width;
    format!(
        "{}…{}",
        slice_width(value, 0, left_width),
        suffix_width(value, right_width)
    )
}

pub(super) fn slice_width(value: &str, skip: usize, width: usize) -> String {
    let mut skipped = 0;
    let mut used = 0;
    let mut output = String::new();
    for character in value.chars() {
        let character_width = character.width().unwrap_or_default();
        if skipped + character_width <= skip {
            skipped += character_width;
            continue;
        }
        if used + character_width > width {
            break;
        }
        output.push(character);
        used += character_width;
    }
    output
}

pub(super) fn suffix_width(value: &str, width: usize) -> String {
    let mut characters = Vec::new();
    let mut used = 0;
    for character in value.chars().rev() {
        let character_width = character.width().unwrap_or_default();
        if used + character_width > width {
            break;
        }
        characters.push(character);
        used += character_width;
    }
    characters.into_iter().rev().collect()
}
