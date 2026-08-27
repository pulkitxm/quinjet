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
    let unknown = count(PullRequestCheckStatus::Unknown);
    let cancelled = count(PullRequestCheckStatus::Cancelled);
    let number = tip.map_or(0, |member| member.number);
    let title = tip.map_or("Stack tip", |member| member.title.as_str());
    let head = tip.map_or_else(String::new, |member| short_oid(&member.head_oid));
    let updated = tip.map_or_else(String::new, |member| {
        format_relative_timestamp(&member.updated_at)
    });
    let stack_error = app.pull_request_stack_error.as_deref();
    let detail = match (stack_error, tip) {
        (Some(error), _) => format!("Stack metadata refresh failed: {error}"),
        (None, None) => format!(
            "{} of {} members loaded · final tip checks unavailable",
            stack.members.len(),
            stack.size
        ),
        (None, Some(_)) => app.stack_inspector.tip_checks_error.as_deref().map_or_else(
            || {
                format!(
                    "{passed} passed · {failed} failed · {running} running · {cancelled} stale · {skipped} skipped · {unknown} unknown"
                )
            },
            ToOwned::to_owned,
        ),
    };
    let heading = stack_gate_heading(app, tip, number);
    let identity = if tip.is_none() {
        "GitHub did not return every stack member".to_owned()
    } else {
        format!("#{number} {title}")
    };
    let state_width = state.label().width().saturating_add(4);
    let identity = truncate_end(
        &identity,
        usize::from(area.width).saturating_sub(state_width),
    );
    let revision = if tip.is_none() {
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
                if tip.is_none() {
                    "[t Final checks unavailable]"
                } else if stack_error.is_some() {
                    "[t Inspect last-known checks]"
                } else {
                    "[t Inspect final checks]"
                },
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if tip.is_none() {
                    " · refresh stack metadata"
                } else if stack_error.is_some() {
                    " · metadata stale"
                } else if app.stack_inspector.tip_checks_loading {
                    " · refreshing final checks"
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

fn stack_gate_heading(
    app: &App,
    tip: Option<&crate::git::github::PullRequestStackMember>,
    number: u64,
) -> String {
    if tip.is_none() && app.pull_request_stack_error.is_some() {
        "STACK GATE / TIP UNAVAILABLE / STALE PARTIAL STACK".to_owned()
    } else if tip.is_none() {
        "STACK GATE / TIP UNAVAILABLE / PARTIAL STACK".to_owned()
    } else if app.pull_request_stack_error.is_some() {
        "STACK GATE / TIP UNVERIFIED / STALE METADATA".to_owned()
    } else if app.stack_inspector.tip_checks_loading && app.stack_inspector.tip_checks_loaded {
        format!("FINAL STACK GATE / TIP #{number} / REFRESHING CHECKS")
    } else {
        format!("FINAL STACK GATE / TIP #{number} / FULL CHANGE SET")
    }
}
