#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn check_summary_line(app: &App, theme: &Theme) -> Line<'static> {
    let count = |status: PullRequestCheckStatus| {
        app.pull_request_checks
            .iter()
            .filter(|check| check.status == status)
            .count()
    };
    let passed = count(PullRequestCheckStatus::Passed);
    let pending = count(PullRequestCheckStatus::Pending);
    let failed = count(PullRequestCheckStatus::Failed);
    let mut spans = vec![Span::styled(
        format!("{:<DETAIL_LABEL_WIDTH$}", "Checks"),
        Style::default().fg(theme.muted),
    )];
    if app.pull_request_checks.is_empty() {
        spans.push(Span::styled(
            if app.pull_request_checks_loading {
                "loading…"
            } else {
                "none reported"
            },
            Style::default().fg(theme.muted),
        ));
        return Line::from(spans);
    }
    spans.push(Span::styled(
        format!("✓{passed}  "),
        Style::default().fg(theme.success),
    ));
    spans.push(Span::styled(
        format!("◌{pending}  "),
        Style::default().fg(theme.accent),
    ));
    spans.push(Span::styled(
        format!("×{failed}"),
        Style::default().fg(theme.error),
    ));
    Line::from(spans)
}

pub(super) fn check_run_rows(app: &App, width: usize, theme: &Theme) -> Vec<ContentRow> {
    let mut rows = Vec::new();
    let Some(check) = app.selected_pull_request_check() else {
        return rows;
    };
    let (icon, color) = pull_request_check_icon(check.status, theme);
    rows.push(ContentRow::wide(Line::from(vec![
        Span::styled(
            format!("{icon} "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            check.name.clone(),
            if check.link.is_empty() {
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
            } else {
                Link::style(theme)
            },
        ),
    ])));
    rows.push(ContentRow::wide(detail_line(
        "Workflow",
        format!("{}  ·  {}", check.workflow, check.state.to_lowercase()),
        theme,
    )));
    if !check.started_at.is_empty() {
        rows.push(ContentRow::wide(detail_line(
            "Ran",
            format!(
                "{}{}",
                format_local_timestamp(&check.started_at),
                match check.duration_label() {
                    duration if duration.is_empty() => String::new(),
                    duration => format!("  ·  {duration}"),
                }
            ),
            theme,
        )));
    }
    if !check.description.is_empty() {
        rows.push(ContentRow::wide(detail_line(
            "Details",
            check.description.clone(),
            theme,
        )));
    }
    if !check.link.is_empty() {
        rows.push(ContentRow::wide(link_detail_line(
            "URL",
            check.link.clone(),
            theme,
        )));
    }
    rows.push(ContentRow::blank());

    if let Some(error) = app.pull_request_check_log_error.as_deref() {
        rows.push(ContentRow::text(
            format!("  {error}"),
            Style::default().fg(theme.error),
        ));
        return rows;
    }
    let Some(log) = app.pull_request_check_log.as_ref() else {
        rows.push(ContentRow::text(
            if app.pull_request_check_log_loading {
                "  Loading the run log…"
            } else {
                "  No log loaded"
            },
            Style::default().fg(theme.muted),
        ));
        return rows;
    };
    if let Some(reason) = log.unavailable.as_deref() {
        rows.push(ContentRow::text(
            format!("  {reason}"),
            Style::default().fg(theme.muted),
        ));
        return rows;
    }
    rows.push(ContentRow::plain(section_rule(
        &format!("{} steps", log.steps.len()),
        width,
        theme,
    )));
    if log.log_pending {
        rows.push(ContentRow::text(
            "  Waiting for the runner to write its first output",
            Style::default().fg(theme.muted),
        ));
    }
    let now = crate::git::github::unix_now();
    for step in &log.steps {
        let expanded = app.check_step_expanded(step.number);
        rows.push(check_step_row(step, expanded, now, width, theme));
        if expanded {
            if step.lines.is_empty() && step.status.is_running() {
                rows.push(ContentRow::text(
                    "   │ waiting for output…",
                    Style::default().fg(theme.accent),
                ));
            } else {
                push_log_lines(&mut rows, &step.lines, theme);
            }
        }
    }
    if !log.loose_lines.is_empty() {
        rows.push(ContentRow::plain(section_rule(
            "Runner output",
            width,
            theme,
        )));
        push_log_lines(&mut rows, &log.loose_lines, theme);
    }
    if log.truncated {
        rows.push(ContentRow::text(
            "  … log truncated to keep Quinjet responsive …",
            Style::default().fg(theme.muted),
        ));
    }
    rows
}

#[doc = " A step reads as one row: fold state, outcome, name, and how long it took,"]
#[doc = " with the duration pushed to the right edge the way a run page shows it."]
pub(super) fn check_step_row(
    step: &CheckStep,
    expanded: bool,
    now: i64,
    width: usize,
    theme: &Theme,
) -> ContentRow {
    let (icon, color) = pull_request_check_icon(step.status, theme);
    let duration = step.duration_label(now);
    let reserved = 8 + duration.width();
    let name = truncate_end(&step.name, width.saturating_sub(reserved));
    let padding = width
        .saturating_sub(reserved)
        .saturating_sub(name.width())
        .saturating_add(1);
    ContentRow {
        line: Line::from(vec![
            Span::styled(
                disclosure_prefix(expanded),
                Style::default().fg(theme.muted),
            ),
            Span::styled(format!("{icon} "), Style::default().fg(color)),
            Span::styled(name, Style::default().fg(theme.text)),
            Span::styled(" ".repeat(padding), Style::default()),
            Span::styled(duration, Style::default().fg(theme.muted)),
        ]),
        step: Some(step.number),
        wide: false,
    }
}

pub(super) fn push_log_lines(rows: &mut Vec<ContentRow>, lines: &[CheckLogLine], theme: &Theme) {
    if lines.is_empty() {
        rows.push(ContentRow::text(
            "   │ no output",
            Style::default().fg(theme.muted),
        ));
        return;
    }
    for line in lines {
        rows.push(ContentRow::wide(Line::from(vec![
            Span::styled("   │ ", Style::default().fg(theme.border)),
            Span::styled(
                line.text.clone(),
                Style::default().fg(log_severity_color(line.severity, theme)),
            ),
        ])));
    }
}

pub(super) const fn log_severity_color(severity: CheckLogSeverity, theme: &Theme) -> Color {
    match severity {
        CheckLogSeverity::Normal => theme.text,
        CheckLogSeverity::Command => theme.accent,
        CheckLogSeverity::Notice | CheckLogSeverity::Warning => theme.modified,
        CheckLogSeverity::Error => theme.error,
    }
}

pub(super) fn section_rule(label: &str, width: usize, theme: &Theme) -> Line<'static> {
    let label = format!(" {label} ");
    let fill = width.saturating_sub(label.width()).saturating_sub(2);
    Line::from(vec![
        Span::styled("──", Style::default().fg(theme.border)),
        Span::styled(label, Style::default().fg(theme.muted)),
        Span::styled("─".repeat(fill), Style::default().fg(theme.border)),
    ])
}

pub(super) fn short_oid(value: &str) -> String {
    value.chars().take(7).collect()
}
