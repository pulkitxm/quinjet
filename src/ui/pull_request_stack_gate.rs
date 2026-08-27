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
    let stack_error = app.pull_request_stack_error.as_deref();
    let mut detail = match (stack_error, tip) {
        (Some(error), _) => format!("metadata stale: {error}"),
        (None, None) => "tip checks unavailable".to_owned(),
        (None, Some(_)) => app.stack_inspector.tip_checks_error.as_deref().map_or_else(
            || {
                format!("{passed} pass · {failed} fail · {running} running · {cancelled} stale · {skipped} skip · {unknown} unknown")
            },
            ToOwned::to_owned,
        ),
    };
    if app.stack_inspector.tip_checks_loading && tip.is_some() && stack_error.is_none() {
        detail.push_str(" · REFRESHING CHECKS");
    }
    let identity = if tip.is_none() {
        "TIP unavailable".to_owned()
    } else {
        format!("#{number} {title}")
    };
    let action = if tip.is_none() {
        "[t Checks unavailable]"
    } else if stack_error.is_some() {
        "[t Inspect last-known checks]"
    } else {
        "[t Inspect tip checks]"
    };
    let gate = if tip.is_none() {
        "GATE UNAVAILABLE"
    } else if stack_error.is_some() {
        "GATE UNVERIFIED"
    } else {
        "FINAL GATE"
    };
    let first = Line::from(vec![
        Span::styled(" ▌ ", Style::default().fg(color)),
        Span::styled(
            format!("{gate} {}", state.label()),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" · {} · {detail}", truncate_end(&identity, 48)),
            Style::default().fg(theme.text),
        ),
    ]);
    let second = Line::from(vec![
        Span::styled(" ▌ ", Style::default().fg(color)),
        Span::styled(
            action,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · cumulative tip CI", Style::default().fg(theme.muted)),
    ]);
    frame.render_widget(
        Paragraph::new(Text::from(vec![first, second])).style(Style::default().bg(theme.panel_alt)),
        area,
    );
    StackInspectorHitArea {
        area,
        target: StackInspectorHit::TipChecks,
    }
}
