#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw_tip_gate(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    stack: &crate::git::github::PullRequestStack,
    theme: &Theme,
) -> StackInspectorHitArea {
    let state = pull_request_stack::stack_gate_state(app);
    let color = state.color(theme);
    let tip = stack.tip();
    let checks = &app.stack_inspector.tip_checks.checks;
    let count = |status| checks.iter().filter(|check| check.status == status).count();
    let passed = count(PullRequestCheckStatus::Passed);
    let failed = count(PullRequestCheckStatus::Failed);
    let running = count(PullRequestCheckStatus::Pending);
    let skipped = count(PullRequestCheckStatus::Skipped);
    let cancelled = count(PullRequestCheckStatus::Cancelled);
    let number = tip.map_or(0, |member| member.number);
    let title = tip.map_or("Stack tip", |member| member.title.as_str());
    let head = tip.map_or_else(String::new, |member| short_oid(&member.head_oid));
    let updated = tip.map_or_else(String::new, |member| {
        format_relative_timestamp(&member.updated_at)
    });
    let detail = if stack.truncated {
        format!(
            "{} of {} members loaded · final tip checks unavailable",
            stack.members.len(),
            stack.size
        )
    } else {
        app.stack_inspector.tip_checks_error.as_deref().map_or_else(
            || {
                format!(
                    "{passed} passed · {failed} failed · {running} running · {cancelled} stale · {skipped} skipped"
                )
            },
            ToOwned::to_owned,
        )
    };
    let heading = if stack.truncated {
        "STACK GATE / TIP UNAVAILABLE / PARTIAL STACK".to_owned()
    } else {
        format!("FINAL STACK GATE / TIP #{number} / FULL CHANGE SET")
    };
    let identity = if stack.truncated {
        "GitHub did not return every stack member".to_owned()
    } else {
        format!("#{number} {title}")
    };
    let state_width = state.label().width().saturating_add(4);
    let identity = truncate_end(
        &identity,
        usize::from(area.width).saturating_sub(state_width),
    );
    let revision = if stack.truncated {
        String::new()
    } else {
        format!(" · head {head} · updated {updated}")
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("▌ ", Style::default().fg(color)),
            Span::styled(
                heading,
                Style::default()
                    .fg(theme.muted)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("▌ ", Style::default().fg(color)),
            Span::styled(
                state.label(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {identity}"),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("▌ ", Style::default().fg(color)),
            Span::styled(detail, Style::default().fg(theme.text)),
            Span::styled(revision, Style::default().fg(theme.muted)),
        ]),
        Line::from(vec![
            Span::styled("▌ ", Style::default().fg(color)),
            Span::styled(
                if stack.truncated {
                    "[t Final checks unavailable]"
                } else {
                    "[t Inspect final checks]"
                },
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if stack.truncated {
                    " · refresh stack metadata"
                } else if app.stack_inspector.tip_checks.from_cache {
                    " · cached"
                } else {
                    " · cumulative tip CI"
                },
                Style::default().fg(theme.muted),
            ),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(Text::from(lines)).style(Style::default().bg(theme.panel_alt)),
        area,
    );
    StackInspectorHitArea {
        area,
        target: StackInspectorHit::TipChecks,
    }
}
