#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw_ssh_project_modal(
    frame: &mut Frame<'_>,
    modal: &Modal,
    context: Option<&SshContext>,
    theme: &Theme,
) {
    match modal {
        Modal::Projects {
            groups,
            selected,
            query,
            loading,
            mode,
        } => draw_projects(
            frame, groups, *selected, query, *loading, *mode, context, theme,
        ),
        Modal::SshMachines {
            items,
            selected,
            current,
            ..
        } => draw_ssh_machines(frame, items, *selected, current, theme),
        _ => {}
    }
}

pub(super) fn draw_ssh_machines(
    frame: &mut Frame<'_>,
    items: &[SshMachine],
    selected: usize,
    current: &str,
    theme: &Theme,
) {
    let height = cells(items.len()).saturating_add(4).max(7);
    let area = centered_rect(
        frame.area().width.saturating_sub(18).min(72),
        height.min(frame.area().height.saturating_sub(10)),
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Switch SSH machine ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let list_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(2),
    );
    let offset = selected.saturating_sub(list_area.height.saturating_sub(1) as usize);
    let lines = items
        .iter()
        .skip(offset)
        .take(list_area.height as usize)
        .enumerate()
        .map(|(visible_index, machine)| {
            let active = offset.saturating_add(visible_index) == selected;
            let background = if active { theme.selected } else { theme.panel };
            let current_marker = if machine.target == current {
                "  current"
            } else {
                ""
            };
            let (glyph, color, status) = if machine.accessible {
                ("●", theme.success, "reachable")
            } else {
                ("●", theme.error, "unavailable")
            };
            Line::from(vec![
                Span::styled(
                    format!(" {glyph} "),
                    Style::default().fg(color).bg(background),
                ),
                Span::styled(
                    format!("{:<20}", machine.target),
                    Style::default()
                        .fg(theme.text)
                        .bg(background)
                        .add_modifier(if active {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(
                    format!("  {status:<11}  used {}{current_marker}", machine.uses),
                    Style::default().fg(theme.muted).bg(background),
                ),
            ])
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), list_area);
    draw_modal_hint(frame, area, "Enter switch   Esc projects", theme);
}
