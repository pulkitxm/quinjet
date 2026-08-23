#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw_theme_picker(
    frame: &mut Frame<'_>,
    selected: usize,
    current: ThemeName,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    let choices = ThemeName::ALL.map(|name| (name.label(), name == current));
    draw_choice_picker(frame, " Select Theme ", &choices, selected, list, theme);
}

pub(super) fn draw_appearance_picker(
    frame: &mut Frame<'_>,
    selected: usize,
    current: AppearanceChoice,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    let choices = AppearanceChoice::ALL.map(|choice| (choice.label(), choice == current));
    draw_choice_picker(
        frame,
        " Select Appearance ",
        &choices,
        selected,
        list,
        theme,
    );
}

fn draw_choice_picker(
    frame: &mut Frame<'_>,
    title: &str,
    choices: &[(&'static str, bool)],
    selected: usize,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    let height = (cells(choices.len()) + 4)
        .min(frame.area().height.saturating_sub(6))
        .max(7);
    let area = centered_rect(44, height, frame.area());
    frame.render_widget(Clear, area);
    let block = modal_block(title, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let list_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(1),
    );
    let offset = list.offset(selected, list_area.height as usize, choices.len());
    let lines = choices
        .iter()
        .skip(offset)
        .take(list_area.height as usize)
        .enumerate()
        .map(|(index, (label, current))| {
            choice_line(label, *current, offset + index == selected, theme)
        })
        .collect::<Vec<_>>();
    for index in 0..lines.len() {
        list.hit(
            Rect::new(
                list_area.x,
                list_area.y.saturating_add(cells(index)),
                list_area.width,
                1,
            ),
            offset.saturating_add(index),
        );
    }
    frame.render_widget(Paragraph::new(lines), list_area);
    draw_modal_hint(frame, area, "Enter apply   Esc close", theme);
}

fn choice_line(label: &'static str, current: bool, selected: bool, theme: &Theme) -> Line<'static> {
    let background = if selected {
        theme.selected
    } else {
        theme.panel
    };
    Line::from(vec![
        Span::styled(
            if selected { " › " } else { "   " },
            Style::default().fg(theme.accent).bg(background),
        ),
        Span::styled(
            if current { "✓ " } else { "  " },
            Style::default().fg(theme.success).bg(background),
        ),
        Span::styled(
            label,
            Style::default()
                .fg(theme.text)
                .bg(background)
                .add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
    ])
}
