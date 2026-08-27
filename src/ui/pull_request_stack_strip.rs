#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;
use crate::git::github::{PullRequestStack, PullRequestStackMember};

pub(super) fn draw_compact_stack_strip(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    stack: &PullRequestStack,
    theme: &Theme,
) -> Vec<SidebarHitArea> {
    frame.render_widget(
        section_separator_block("REVIEW PATH · p/n select · Shift+↑/↓ range", theme),
        Rect::new(area.x, area.y, area.width, area.height.min(1)),
    );
    let cursor = stack
        .members
        .iter()
        .position(|member| Some(member.position) == app.pull_request_stack_cursor)
        .unwrap_or_default();
    if app.sidebar_last_cursor != Some(cursor) {
        app.sidebar_free_scroll = false;
    }
    let start = if app.sidebar_free_scroll {
        let last = stack.members.len().saturating_sub(1);
        app.sidebar_offset
            .min(compact_stack_start(stack, last, area.width))
    } else {
        compact_stack_start(stack, cursor, area.width)
    };
    app.sidebar_offset = start;
    app.sidebar_last_cursor = Some(cursor);
    let mut x = area.x;
    let y = area.y.saturating_add(area.height.saturating_sub(1));
    let mut hits = Vec::new();
    for member in stack.members.iter().skip(start) {
        let label = compact_stack_label(member, stack);
        let width = cells(label.width()).min(area.right().saturating_sub(x));
        let row = Rect::new(x, y, width, 1);
        frame.render_widget(
            Paragraph::new(label).style(
                Style::default()
                    .fg(if Some(member.position) == app.pull_request_stack_cursor {
                        theme.text
                    } else {
                        theme.muted
                    })
                    .bg(if Some(member.position) == app.pull_request_stack_cursor {
                        theme.selected
                    } else {
                        theme.panel
                    }),
            ),
            row,
        );
        hits.push(SidebarHitArea {
            area: row,
            target: SidebarHit::PullRequestStackMember(member.position),
        });
        x = x.saturating_add(width);
        if x >= area.right() {
            break;
        }
    }
    hits
}

fn compact_stack_start(stack: &PullRequestStack, cursor: usize, width: u16) -> usize {
    let mut start = cursor.min(stack.members.len().saturating_sub(1));
    let mut used = stack
        .members
        .get(start)
        .map_or(0, |member| compact_stack_label(member, stack).width());
    while start > 0 {
        let previous = start.saturating_sub(1);
        let previous_width = stack
            .members
            .get(previous)
            .map_or(0, |member| compact_stack_label(member, stack).width());
        if used.saturating_add(previous_width) > usize::from(width) {
            break;
        }
        start = previous;
        used = used.saturating_add(previous_width);
    }
    start
}

fn compact_stack_label(member: &PullRequestStackMember, stack: &PullRequestStack) -> String {
    let tip = if stack.tip() == Some(member) {
        " TIP"
    } else {
        ""
    };
    format!(" {} #{}{tip} ", member.position, member.number)
}
