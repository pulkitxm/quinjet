#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw_pull_request_stack(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
) -> Vec<SidebarHitArea> {
    let Some(stack) = app.pull_request_stack.clone() else {
        frame.render_widget(
            Paragraph::new(" Stack metadata unavailable")
                .style(Style::default().fg(theme.muted).bg(theme.panel)),
            area,
        );
        return Vec::new();
    };
    let Some((from, to)) = app.pull_request_stack_range() else {
        return Vec::new();
    };
    let heading = Rect::new(area.x, area.y, area.width, area.height.min(1));
    let partial = if stack.truncated { " · partial" } else { "" };
    frame.render_widget(
        Paragraph::new(format!(" {} · range {from}..{to}{partial}", stack.base_ref))
            .style(Style::default().fg(theme.muted).bg(theme.panel_alt)),
        heading,
    );
    let list_area = Rect::new(
        area.x,
        area.y.saturating_add(1),
        area.width,
        area.height.saturating_sub(1),
    );
    let members = stack.members;
    let cursor_index = members
        .iter()
        .position(|member| Some(member.position) == app.pull_request_stack_cursor)
        .unwrap_or_default();
    app.sidebar_viewport(cursor_index, usize::from(list_area.height), members.len());
    let start = app.sidebar_offset;
    members
        .iter()
        .skip(start)
        .take(usize::from(list_area.height))
        .enumerate()
        .map(|(visible_index, member)| {
            let row = Rect::new(
                list_area.x,
                list_area
                    .y
                    .saturating_add(u16::try_from(visible_index).unwrap_or_default()),
                list_area.width,
                1,
            );
            let cursor = Some(member.position) == app.pull_request_stack_cursor;
            let in_range = (from..=to).contains(&member.position);
            let marker = if cursor {
                "›"
            } else if in_range {
                "●"
            } else {
                "│"
            };
            let state = if member.is_draft {
                "DRAFT"
            } else {
                member.state.as_str()
            };
            let prefix = format!(" {marker} {} #{} ", member.position, member.number);
            let title_width = usize::from(row.width)
                .saturating_sub(prefix.width())
                .saturating_sub(state.width())
                .saturating_sub(1);
            let row_style = Style::default().bg(if cursor {
                theme.selected
            } else if in_range {
                theme.panel_alt
            } else {
                theme.panel
            });
            let marker_style = Style::default()
                .fg(if in_range { theme.accent } else { theme.border })
                .add_modifier(if cursor {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });
            let state_style = Style::default().fg(match state {
                "OPEN" => theme.success,
                "MERGED" => theme.accent,
                "CLOSED" => theme.removed,
                _ => theme.modified,
            });
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(prefix, marker_style),
                    Span::styled(
                        truncate_end(&member.title, title_width),
                        Style::default().fg(theme.text),
                    ),
                    Span::raw(" "),
                    Span::styled(state.to_owned(), state_style),
                ]))
                .style(row_style),
                row,
            );
            SidebarHitArea {
                area: row,
                target: SidebarHit::PullRequestStackMember(member.position),
            }
        })
        .collect()
}
