#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;
use crate::git::github::PullRequestStackMember;

const WIDE_STACK_WORKSPACE: u16 = 104;

#[derive(Clone, Copy)]
struct StackRailMemberState {
    selected: bool,
    in_range: bool,
    tip: bool,
}

#[derive(Default)]
pub(super) struct StackWorkspaceGeometry {
    pub sidebar: Rect,
    pub sidebar_divider: Rect,
    pub content: Rect,
    pub diff_divider: Option<Rect>,
    pub sidebar_hits: Vec<SidebarHitArea>,
    pub stack_inspector_hits: Vec<StackInspectorHitArea>,
    pub content_file_hits: Vec<ContentFileHit>,
    pub content_step_hits: Vec<ContentStepHit>,
    pub content_review_hits: Vec<ContentReviewHit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StackGateState {
    Pass,
    Fail,
    Running,
    Stale,
    Skip,
    Blocked,
}

impl StackGateState {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Running => "RUNNING",
            Self::Stale => "STALE",
            Self::Skip => "SKIP",
            Self::Blocked => "BLOCKED",
        }
    }

    pub(super) const fn color(self, theme: &Theme) -> Color {
        match self {
            Self::Pass => theme.success,
            Self::Fail | Self::Blocked => theme.error,
            Self::Running => theme.modified,
            Self::Stale => theme.conflict,
            Self::Skip => theme.muted,
        }
    }
}

pub(super) fn draw_pull_request_stack_workspace(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    theme: &Theme,
    link_hits: &mut Vec<LinkHit>,
) -> StackWorkspaceGeometry {
    let Some(stack) = app.pull_request_stack.clone() else {
        return StackWorkspaceGeometry::default();
    };
    let heading_height = if area.height >= 14 { 2 } else { 1 };
    let gate_height = if area.height >= 16 {
        4
    } else if area.height >= 12 {
        3
    } else {
        2
    };
    let [heading, gate, workspace] = Layout::vertical([
        Constraint::Length(heading_height),
        Constraint::Length(gate_height),
        Constraint::Min(4),
    ])
    .areas(area);
    let mut stack_inspector_hits = draw_stack_heading(frame, heading, app, &stack, theme);
    stack_inspector_hits.push(pull_request_stack_gate::draw_tip_gate(
        frame, gate, app, &stack, theme,
    ));

    let wide = workspace.width >= WIDE_STACK_WORKSPACE;
    let (sidebar, sidebar_divider, content, sidebar_hits) = if wide {
        let maximum = workspace.width.saturating_sub(44).max(30);
        let rail_width = app.sidebar_width.clamp(30, maximum);
        let [rail, divider, detail] = Layout::horizontal([
            Constraint::Length(rail_width),
            Constraint::Length(1),
            Constraint::Min(43),
        ])
        .areas(workspace);
        let hits = draw_stack_rail(frame, rail, app, &stack, theme);
        draw_main_divider(frame, divider, app.resize_target.is_some(), theme);
        (rail, divider, detail, hits)
    } else {
        let [strip, detail] =
            Layout::vertical([Constraint::Length(2), Constraint::Min(3)]).areas(workspace);
        let hits =
            pull_request_stack_strip::draw_compact_stack_strip(frame, strip, app, &stack, theme);
        (strip, Rect::default(), detail, hits)
    };

    let (diff_divider, content_file_hits, content_step_hits, content_review_hits) =
        if app.stack_inspector.diff_open {
            draw_content(frame, content, app, theme, link_hits)
        } else {
            pull_request_stack_detail::draw_stack_member_detail(
                frame,
                content,
                app,
                theme,
                &mut stack_inspector_hits,
                link_hits,
            );
            (None, Vec::new(), Vec::new(), Vec::new())
        };
    StackWorkspaceGeometry {
        sidebar,
        sidebar_divider,
        content,
        diff_divider,
        sidebar_hits,
        stack_inspector_hits,
        content_file_hits,
        content_step_hits,
        content_review_hits,
    }
}

fn draw_stack_heading(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    stack: &crate::git::github::PullRequestStack,
    theme: &Theme,
) -> Vec<StackInspectorHitArea> {
    let tip = stack.tip();
    let partial = if stack.truncated { " · PARTIAL" } else { "" };
    let tip = tip.map_or_else(
        || "TIP ?".to_owned(),
        |member| format!("TIP #{}", member.number),
    );
    let status = if app.pull_request_stack_error.is_some() {
        " · STALE METADATA".to_owned()
    } else if app.pull_request_stack_loading {
        " · REFRESHING".to_owned()
    } else if app.pull_request_warnings.is_empty() {
        String::new()
    } else {
        format!(" · WARN {}", app.pull_request_warnings.len())
    };
    let first = format!(
        " STACK #{}{status} · {} PR{} · {} -> {tip}{partial}",
        stack.number,
        stack.size,
        if stack.size == 1 { "" } else { "S" },
        stack.base_ref,
    );
    frame.render_widget(
        Paragraph::new(truncate_end(&first, usize::from(area.width))).style(
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        ),
        Rect::new(area.x, area.y, area.width, area.height.min(1)),
    );
    let Some((from, to)) = app.pull_request_stack_range() else {
        return Vec::new();
    };
    let action = if app.stack_inspector.diff_open {
        " [d Diff open] "
    } else {
        " [d Open range diff] "
    };
    let action_width = cells(action.width()).min(area.width);
    let action_area = Rect::new(
        area.right().saturating_sub(action_width),
        area.y.saturating_add(1),
        action_width,
        area.height.saturating_sub(1).min(1),
    );
    let range_area = Rect::new(
        area.x,
        area.y.saturating_add(1),
        area.width.saturating_sub(action_width),
        area.height.saturating_sub(1).min(1),
    );
    frame.render_widget(
        Paragraph::new(format!(" RANGE {from}..{to} · base({from}) -> head({to})"))
            .style(Style::default().fg(theme.muted).bg(theme.panel_alt)),
        range_area,
    );
    frame.render_widget(
        Paragraph::new(action).style(
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel_alt)
                .add_modifier(Modifier::BOLD),
        ),
        action_area,
    );
    vec![StackInspectorHitArea {
        area: action_area,
        target: StackInspectorHit::Diff,
    }]
}

pub(super) fn stack_gate_state(app: &App) -> StackGateState {
    let checks = &app.stack_inspector.tip_checks.checks;
    let tip = app
        .pull_request_stack
        .as_ref()
        .and_then(crate::git::github::PullRequestStack::tip);
    if app.pull_request_stack_error.is_some() || tip.is_none() {
        return StackGateState::Blocked;
    }
    if checks
        .iter()
        .any(|check| check.status == PullRequestCheckStatus::Failed)
    {
        return StackGateState::Fail;
    }
    if checks
        .iter()
        .any(|check| check.status == PullRequestCheckStatus::Pending)
        || (app.stack_inspector.tip_checks_loading && !app.stack_inspector.tip_checks_loaded)
    {
        return StackGateState::Running;
    }
    if checks.iter().any(|check| {
        check.status == PullRequestCheckStatus::Cancelled
            || check.state.eq_ignore_ascii_case("stale")
    }) {
        return StackGateState::Stale;
    }
    if tip.is_some_and(stack_member_blocked) || app.stack_inspector.tip_checks_error.is_some() {
        return StackGateState::Blocked;
    }
    if app.stack_inspector.tip_checks_loaded {
        if checks
            .iter()
            .any(|check| check.status == PullRequestCheckStatus::Unknown)
        {
            return StackGateState::Blocked;
        }
        if checks.is_empty()
            || checks
                .iter()
                .all(|check| check.status == PullRequestCheckStatus::Skipped)
        {
            return StackGateState::Skip;
        }
        return StackGateState::Pass;
    }
    match tip.map_or("", |member| member.checks_state.as_str()) {
        "FAILURE" | "ERROR" => StackGateState::Fail,
        "PENDING" | "EXPECTED" => StackGateState::Running,
        "STALE" => StackGateState::Stale,
        "SUCCESS" => StackGateState::Pass,
        _ => StackGateState::Skip,
    }
}

fn stack_member_blocked(member: &PullRequestStackMember) -> bool {
    member.is_draft
        || member.mergeable.eq_ignore_ascii_case("conflicting")
        || member
            .review_decision
            .eq_ignore_ascii_case("changes_requested")
        || matches!(member.merge_state.as_str(), "BEHIND" | "BLOCKED" | "DIRTY")
}

fn draw_stack_rail(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    stack: &crate::git::github::PullRequestStack,
    theme: &Theme,
) -> Vec<SidebarHitArea> {
    frame.render_widget(
        Paragraph::new(" BASE -> TIP · MEMBER HEALTH")
            .style(Style::default().fg(theme.muted).bg(theme.panel_alt)),
        Rect::new(area.x, area.y, area.width, area.height.min(1)),
    );
    let list = Rect::new(
        area.x,
        area.y.saturating_add(1),
        area.width,
        area.height.saturating_sub(1),
    );
    let capacity = usize::from(list.height).div_euclid(2);
    let cursor = stack
        .members
        .iter()
        .position(|member| Some(member.position) == app.pull_request_stack_cursor)
        .unwrap_or_default();
    app.sidebar_viewport(cursor, capacity, stack.members.len());
    let start = app.sidebar_offset;
    let range = app.pull_request_stack_range();
    stack
        .members
        .iter()
        .skip(start)
        .take(capacity)
        .enumerate()
        .map(|(visible, member)| {
            let row = Rect::new(
                list.x,
                list.y.saturating_add(cells(visible.saturating_mul(2))),
                list.width,
                2,
            );
            draw_stack_rail_member(
                frame,
                row,
                member,
                StackRailMemberState {
                    selected: Some(member.position) == app.pull_request_stack_cursor,
                    in_range: range
                        .is_some_and(|(from, to)| (from..=to).contains(&member.position)),
                    tip: stack.tip() == Some(member),
                },
                theme,
            );
            SidebarHitArea {
                area: row,
                target: SidebarHit::PullRequestStackMember(member.position),
            }
        })
        .collect()
}

fn draw_stack_rail_member(
    frame: &mut Frame<'_>,
    area: Rect,
    member: &PullRequestStackMember,
    member_state: StackRailMemberState,
    theme: &Theme,
) {
    let background = if member_state.selected {
        theme.selected
    } else if member_state.in_range {
        theme.panel_alt
    } else {
        theme.panel
    };
    let marker = if member_state.selected {
        ">"
    } else if member_state.in_range {
        "●"
    } else {
        "│"
    };
    let tip = if member_state.tip { " TIP" } else { "" };
    let state = if member.is_draft {
        "DRAFT"
    } else {
        member.state.as_str()
    };
    let prefix = format!(" {marker} {} #{} ", member.position, member.number);
    let suffix = format!(" {tip} {state}");
    let title = truncate_end(
        &member.title,
        usize::from(area.width)
            .saturating_sub(prefix.width())
            .saturating_sub(suffix.width()),
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(prefix, Style::default().fg(theme.accent)),
            Span::styled(title, Style::default().fg(theme.text)),
            Span::styled(
                suffix,
                Style::default().fg(member_state_color(state, theme)),
            ),
        ]))
        .style(Style::default().bg(background)),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let health = member_health_label(member);
    let signal = format!(
        "   {} · CI {health} · +{} -{}",
        member.review_decision.replace('_', " "),
        member.additions,
        member.deletions,
    );
    frame.render_widget(
        Paragraph::new(truncate_end(&signal, usize::from(area.width))).style(
            Style::default()
                .fg(member_health_color(health, theme))
                .bg(background),
        ),
        Rect::new(area.x, area.y.saturating_add(1), area.width, 1),
    );
}

fn member_health_label(member: &PullRequestStackMember) -> &'static str {
    if stack_member_blocked(member) {
        return "BLOCKED";
    }
    match member.checks_state.as_str() {
        "SUCCESS" => "PASS",
        "FAILURE" | "ERROR" => "FAIL",
        "PENDING" | "EXPECTED" => "RUNNING",
        "STALE" => "STALE",
        _ => "SKIP",
    }
}

const fn member_health_color(health: &str, theme: &Theme) -> Color {
    match health.as_bytes() {
        b"PASS" => theme.success,
        b"FAIL" | b"BLOCKED" => theme.error,
        b"RUNNING" => theme.modified,
        b"STALE" => theme.conflict,
        _ => theme.muted,
    }
}

const fn member_state_color(state: &str, theme: &Theme) -> Color {
    match state.as_bytes() {
        b"OPEN" => theme.success,
        b"MERGED" => theme.accent,
        b"CLOSED" => theme.removed,
        _ => theme.modified,
    }
}
