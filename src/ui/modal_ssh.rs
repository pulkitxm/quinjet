#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw_project_machines(
    frame: &mut Frame<'_>,
    area: Rect,
    context: &SshContext,
    focused: Option<usize>,
    theme: &Theme,
) -> Vec<(Rect, usize)> {
    let label = Rect::new(area.x, area.y, 9_u16.min(area.width), 1);
    frame.render_widget(
        Paragraph::new(" Machine ").style(Style::default().fg(theme.muted)),
        label,
    );
    let mut x = label.right();
    let mut hits = Vec::new();
    for (index, machine) in context.machines.iter().enumerate() {
        let remaining = area.right().saturating_sub(x);
        if remaining < 6 {
            break;
        }
        let desired = cells(machine.target.width().saturating_add(5));
        let width = desired.min(remaining);
        let rect = Rect::new(x, area.y, width, 1);
        let selected = focused == Some(index);
        let current = machine.target == context.current;
        let background = if selected {
            theme.selected
        } else if current {
            theme.accent
        } else {
            theme.panel_alt
        };
        let foreground = if current && !selected {
            theme.background
        } else {
            theme.text
        };
        let marker_color = if current && !selected {
            theme.background
        } else if machine.accessible {
            theme.success
        } else {
            theme.error
        };
        let marker = if current {
            "✓"
        } else if machine.accessible {
            "●"
        } else {
            "○"
        };
        let target_width = width.saturating_sub(4) as usize;
        let modifier = if selected || current {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!(" {marker} "),
                    Style::default()
                        .fg(marker_color)
                        .bg(background)
                        .add_modifier(modifier),
                ),
                Span::styled(
                    format!("{} ", truncate_middle(&machine.target, target_width)),
                    Style::default()
                        .fg(foreground)
                        .bg(background)
                        .add_modifier(modifier),
                ),
            ])),
            rect,
        );
        hits.push((rect, index));
        x = rect.right().saturating_add(1);
    }
    hits
}

pub(super) fn draw_project_opening(frame: &mut Frame<'_>, path: &Path, area: Rect, theme: &Theme) {
    let width = area.width.saturating_sub(4) as usize;
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("◐ ", Style::default().fg(theme.accent)),
                Span::styled("Opening project…", Style::default().fg(theme.text)),
            ]),
            Line::from(Span::styled(
                truncate_middle(path.to_string_lossy().as_ref(), width),
                Style::default().fg(theme.muted),
            )),
        ]),
        area,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "the shared picker needs modal state, hit targets, machine context, and its theme"
)]
pub(super) fn draw_ssh_project_modal(
    frame: &mut Frame<'_>,
    modal: &Modal,
    collapse_hits: &mut Vec<(Rect, std::path::PathBuf)>,
    context: Option<&SshContext>,
    machine_focus: Option<usize>,
    hits: &mut Vec<(Rect, ModalAction)>,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    if let Modal::Projects {
        groups,
        selected,
        query,
        collapsed,
        loading,
        opening,
        mode,
    } = modal
    {
        hits.extend(
            draw_projects(
                frame,
                collapse_hits,
                groups,
                *selected,
                query,
                collapsed,
                *loading,
                opening.as_deref(),
                *mode,
                context,
                machine_focus,
                list,
                theme,
            )
            .into_iter()
            .map(|(area, index)| (area, ModalAction::SwitchSshMachine(index))),
        );
    }
}
