#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw_accounts(
    frame: &mut Frame<'_>,
    picker: &crate::app::account_picker::AccountPicker,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    let height = cells(picker.items.len()).saturating_add(5).max(7);
    let area = centered_rect(
        frame.area().width.saturating_sub(4).min(68),
        height.min(frame.area().height.saturating_sub(4)),
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" GitHub accounts ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let list_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(2),
    );
    let message = if !picker.ready {
        Some("Loading GitHub accounts…")
    } else if let Some(error) = picker.message.as_deref() {
        Some(error)
    } else if picker.items.is_empty() {
        Some("No saved accounts. Add one with gh auth login.")
    } else {
        None
    };
    if let Some(message) = message {
        frame.render_widget(
            Paragraph::new(message).style(Style::default().fg(theme.muted)),
            list_area,
        );
    } else {
        let offset = list.offset(
            picker.selected,
            usize::from(list_area.height),
            picker.items.len(),
        );
        for (row, login) in picker
            .items
            .iter()
            .skip(offset)
            .take(usize::from(list_area.height))
            .enumerate()
        {
            let index = offset.saturating_add(row);
            let active = index == picker.selected;
            let background = if active { theme.selected } else { theme.panel };
            let marker = if picker.current.as_ref() == Some(login) {
                "  (current)"
            } else {
                ""
            };
            let row_area = Rect::new(
                list_area.x,
                list_area.y.saturating_add(cells(row)),
                list_area.width,
                1,
            );
            frame.render_widget(
                Paragraph::new(format!(
                    "{}@{login}{marker}",
                    if active { " › " } else { "   " }
                ))
                .style(Style::default().fg(theme.text).bg(background)),
                row_area,
            );
            list.hit(row_area, index);
        }
    }
    draw_modal_hint(
        frame,
        area,
        "↑/↓ choose  Home/End jump  Enter switch  Esc cancel",
        theme,
    );
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    #[test]
    fn account_list_scrolls_to_selection_and_marks_current_login() {
        let picker = crate::app::account_picker::AccountPicker {
            items: (0..30).map(|index| format!("account{index:02}")).collect(),
            selected: 29,
            current: Some("account29".to_owned()),
            ready: true,
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        let mut hits = Vec::new();
        let mut len = 0;
        let mut max_scroll = 0;
        let rendered = terminal
            .draw(|frame| {
                let mut list = ModalList::new(&mut hits, &mut len, &mut max_scroll, 0, false);
                draw_accounts(frame, &picker, &mut list, &Theme::default());
            })
            .unwrap();
        let buffer = rendered.buffer;
        let text: String = buffer
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(text.contains("@account29  (current)"));
        assert!(!text.contains("@account00"));
        assert_eq!(len, 30);
        assert!(max_scroll > 0);
        assert_eq!(hits.last().map(|(_, index)| *index), Some(29));
    }
}
