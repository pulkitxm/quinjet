use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;
use crate::git::github::{PullRequestChecks, PullRequestStack, PullRequestStackMember};
use crate::tabs::RepositoryTabs;

fn stack() -> PullRequestStack {
    PullRequestStack {
        node_id: "stack-node".to_owned(),
        number: 19,
        base_ref: "main".to_owned(),
        size: 3,
        selected_position: 3,
        members: (1..=3)
            .map(|position| PullRequestStackMember {
                node_id: format!("pr-node-{position}"),
                entry_id: format!("entry-{position}"),
                position,
                number: 40 + u64::try_from(position).unwrap_or_default(),
                title: format!("Build layer {position}"),
                author: "octocat".to_owned(),
                state: if position == 1 { "MERGED" } else { "OPEN" }.to_owned(),
                is_draft: false,
                updated_at: "2026-08-20T10:00:00Z".to_owned(),
                url: format!("https://github.com/acme/widget/pull/{}", 40 + position),
                base_ref: "main".to_owned(),
                base_oid: format!("{position:040x}"),
                head_ref: format!("layer-{position}"),
                head_oid: format!("{:040x}", position + 10),
                head_repository: Some("acme/widget".to_owned()),
                is_cross_repository: false,
                additions: position * 10,
                deletions: position,
                changed_files: position,
                merge_state: "CLEAN".to_owned(),
                mergeable: "MERGEABLE".to_owned(),
                review_decision: "APPROVED".to_owned(),
                checks_state: "SUCCESS".to_owned(),
                is_queued: false,
            })
            .collect(),
        truncated: false,
        repository: GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://github.com/acme/widget".to_owned(),
            remotes: vec!["origin".to_owned()],
        },
    }
}

fn stack_app() -> App {
    let mut app = overview_app();
    let stack = stack();
    let mut selected = stack.member_pull_request(3).unwrap();
    selected.description = "Keeps permission refresh work bounded across the stack.".to_owned();
    selected.created_at = "2026-08-19T10:00:00Z".to_owned();
    app.pull_request_stack_anchor = Some(2);
    app.pull_request_stack_cursor = Some(3);
    app.pull_request_section = PullRequestSection::Stack;
    app.stack_inspector.selected_locator = stack.member_pull_request(3);
    app.stack_inspector.selected_pull_request = Some(selected);
    app.stack_inspector.tip_locator = stack.member_pull_request(3);
    app.stack_inspector.tip_checks = PullRequestChecks {
        checks: app.pull_request_checks.clone(),
        from_cache: false,
    };
    app.stack_inspector.tip_checks_loaded = true;
    app.stack_inspector.checks = app.stack_inspector.tip_checks.clone();
    app.stack_inspector.checks_loaded = true;
    app.pull_request_stack = Some(stack);
    app
}

fn rendered(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[test]
fn wide_stack_inspector_renders_the_gate_rail_detail_and_hits() {
    let mut app = stack_app();
    let mut terminal = Terminal::new(TestBackend::new(140, 30)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let rendered = rendered(&terminal);
    assert!(rendered.contains("STACK #19"));
    assert!(rendered.contains("FINAL STACK GATE / TIP #43"));
    assert!(rendered.contains("FAIL"));
    assert!(rendered.contains("RANGE 2..3"));
    assert!(rendered.contains("BASE -> TIP · MEMBER HEALTH"));
    assert!(rendered.contains("1 Summary"));
    assert!(rendered.contains("Keeps permission refresh work bounded"));
    assert!(!rendered.contains("Files 3"));
    assert!(app.geometry.sidebar.right() < app.geometry.content.right());
    assert_eq!(app.geometry.sidebar_divider.width, 1);
    assert_eq!(
        app.geometry
            .sidebar_hits
            .iter()
            .filter(|hit| matches!(hit.target, SidebarHit::PullRequestStackMember(_)))
            .count(),
        3
    );
    assert_eq!(
        app.geometry
            .stack_inspector_hits
            .iter()
            .filter(|hit| matches!(hit.target, StackInspectorHit::Section(_)))
            .count(),
        4
    );
    assert!(
        app.geometry
            .stack_inspector_hits
            .iter()
            .any(|hit| hit.target == StackInspectorHit::TipChecks)
    );
    assert!(
        app.geometry
            .stack_inspector_hits
            .iter()
            .any(|hit| hit.target == StackInspectorHit::Diff)
    );
}

#[test]
fn narrow_stack_inspector_replaces_the_rail_with_a_member_strip() {
    let mut app = stack_app();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let rendered = rendered(&terminal);
    assert!(rendered.contains("MEMBERS · [ / ] select"));
    assert!(rendered.contains("3 #43 TIP"));
    assert!(rendered.contains("1 Summary"));
    assert!(!rendered.contains("MEMBER HEALTH"));
    assert!(app.geometry.sidebar.bottom() <= app.geometry.content.y);
    assert_eq!(app.geometry.sidebar_divider, Rect::default());
    assert_eq!(
        app.geometry
            .sidebar_hits
            .iter()
            .filter(|hit| matches!(hit.target, SidebarHit::PullRequestStackMember(_)))
            .count(),
        3
    );
    app.sidebar_offset = 2;
    app.sidebar_free_scroll = true;
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert_eq!(app.sidebar_offset, 0);
}

#[test]
fn compact_stack_strip_keeps_a_wide_selected_member_visible() {
    let mut app = stack_app();
    let mut stack = stack();
    let template = stack.members.first().unwrap().clone();
    stack.members = (1..=8)
        .map(|position| PullRequestStackMember {
            position,
            number: 10_000_000_000_000_000 + u64::try_from(position).unwrap(),
            title: format!("Layer {position}"),
            ..template.clone()
        })
        .collect();
    stack.size = 8;
    stack.selected_position = 8;
    app.pull_request_stack_anchor = Some(8);
    app.pull_request_stack_cursor = Some(8);
    app.pull_request_stack = Some(stack);
    let mut terminal = Terminal::new(TestBackend::new(72, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    assert!(
        app.geometry.sidebar_hits.iter().any(|hit| {
            matches!(hit.target, SidebarHit::PullRequestStackMember(8)) && hit.area.width > 0
        }),
        "{:?}",
        app.geometry.sidebar_hits
    );
}

#[test]
fn compact_stack_strip_preserves_free_scroll_for_mouse_selection() {
    let mut app = stack_app();
    let mut stack = stack();
    let template = stack.members.first().unwrap().clone();
    stack.members = (1..=9)
        .map(|position| PullRequestStackMember {
            position,
            number: 10_000_000_000_000_000 + u64::try_from(position).unwrap_or_default(),
            title: format!("Layer {position}"),
            ..template.clone()
        })
        .collect();
    stack.size = 9;
    stack.selected_position = 1;
    app.pull_request_stack_anchor = Some(1);
    app.pull_request_stack_cursor = Some(1);
    app.pull_request_stack = Some(stack);
    let mut terminal = Terminal::new(TestBackend::new(72, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let sidebar = app.geometry.sidebar;
    drop(app.handle_mouse(
        crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::ScrollDown,
            column: sidebar.x.saturating_add(1),
            row: sidebar.y.saturating_add(1),
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
        std::time::Instant::now(),
    ));
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    assert_eq!(app.sidebar_offset, 2);
    let revealed = app
        .geometry
        .sidebar_hits
        .iter()
        .find(|hit| matches!(hit.target, SidebarHit::PullRequestStackMember(3)))
        .unwrap()
        .area;
    assert!(
        !app.geometry
            .sidebar_hits
            .iter()
            .any(|hit| matches!(hit.target, SidebarHit::PullRequestStackMember(1)))
    );
    drop(app.handle_mouse(
        crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: revealed.x,
            row: revealed.y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
        std::time::Instant::now(),
    ));
    assert_eq!(app.pull_request_stack_cursor, Some(3));
}

#[test]
fn minimum_terminal_height_keeps_stack_member_sections_visible() {
    let mut app = stack_app();
    let mut tabs = RepositoryTabs::new("repo", "/tmp/repo", ());
    let second = tabs.append("second", "/tmp/second", ());
    assert!(tabs.activate(second));
    app.set_repository_tabs(tabs.infos());
    let mut terminal = Terminal::new(TestBackend::new(72, 18)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    assert!(rendered(&terminal).contains("1 Summary"));
    assert!(app.geometry.content.height >= 6);
}

#[test]
fn narrow_gate_keeps_status_visible_with_a_long_tip_title() {
    let mut app = stack_app();
    app.pull_request_stack
        .as_mut()
        .unwrap()
        .members
        .last_mut()
        .unwrap()
        .title =
        "A very long cumulative stack tip title that fills the available terminal width".repeat(3);
    let mut terminal = Terminal::new(TestBackend::new(72, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    assert!(rendered(&terminal).contains("FAIL  #43"));
}

#[test]
fn truncated_stack_blocks_the_gate_without_claiming_a_final_tip() {
    let mut app = stack_app();
    let stack = app.pull_request_stack.as_mut().unwrap();
    stack.size = 5;
    stack.truncated = true;
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let output = rendered(&terminal);
    assert!(output.contains("TIP ? · PARTIAL"));
    assert!(output.contains("STACK GATE / TIP UNAVAILABLE / PARTIAL STACK"));
    assert!(output.contains("BLOCKED"));
    assert!(!output.contains("FINAL STACK GATE"));
    assert!(!output.contains("3 #43 TIP"));

    app.pull_request_stack_error = Some("refresh failed".to_owned());
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let stale_output = rendered(&terminal);
    assert!(stale_output.contains("TIP UNAVAILABLE / STALE PARTIAL STACK"));
    assert!(stale_output.contains("[t Final checks unavailable]"));
    assert!(!stale_output.contains("Inspect last-known checks"));
}

#[test]
fn stack_tip_gate_distinguishes_each_operational_state() {
    let mut app = stack_app();
    let check = app.pull_request_checks.first().unwrap().clone();
    let state_for = |app: &mut App, status: PullRequestCheckStatus| {
        let mut check = check.clone();
        check.status = status;
        check.state = format!("{status:?}").to_uppercase();
        app.stack_inspector.tip_checks.checks = vec![check];
        pull_request_stack::stack_gate_state(app)
    };

    assert_eq!(
        state_for(&mut app, PullRequestCheckStatus::Failed),
        pull_request_stack::StackGateState::Fail
    );
    assert_eq!(
        state_for(&mut app, PullRequestCheckStatus::Pending),
        pull_request_stack::StackGateState::Running
    );
    assert_eq!(
        state_for(&mut app, PullRequestCheckStatus::Cancelled),
        pull_request_stack::StackGateState::Stale
    );
    assert_eq!(
        state_for(&mut app, PullRequestCheckStatus::Skipped),
        pull_request_stack::StackGateState::Skip
    );
    assert_eq!(
        state_for(&mut app, PullRequestCheckStatus::Passed),
        pull_request_stack::StackGateState::Pass
    );
    assert_eq!(
        state_for(&mut app, PullRequestCheckStatus::Unknown),
        pull_request_stack::StackGateState::Blocked
    );
    assert_eq!(
        state_for(&mut app, PullRequestCheckStatus::Passed),
        pull_request_stack::StackGateState::Pass
    );
    app.stack_inspector.tip_checks_loading = true;
    assert_eq!(
        pull_request_stack::stack_gate_state(&app),
        pull_request_stack::StackGateState::Pass
    );
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(rendered(&terminal).contains("REFRESHING CHECKS"));
    app.pull_request_stack_error = Some("stack metadata refresh failed".to_owned());
    assert_eq!(
        pull_request_stack::stack_gate_state(&app),
        pull_request_stack::StackGateState::Blocked
    );
    app.stack_inspector.tip_checks.checks.clear();
    app.stack_inspector.tip_checks_loaded = false;
    app.pull_request_stack
        .as_mut()
        .unwrap()
        .members
        .last_mut()
        .unwrap()
        .merge_state = "BLOCKED".to_owned();
    assert_eq!(
        pull_request_stack::stack_gate_state(&app),
        pull_request_stack::StackGateState::Blocked
    );
}

#[test]
fn unknown_checks_use_muted_rows_and_block_the_gate() {
    let mut app = stack_app();
    let mut check = app.stack_inspector.checks.checks.first().unwrap().clone();
    check.status = PullRequestCheckStatus::Unknown;
    check.state = "UNKNOWN".to_owned();
    app.stack_inspector.checks.checks = vec![check];
    app.stack_inspector.section = StackMemberSection::Checks;
    let theme = Theme::default();

    let (rows, _) = pull_request_stack_rows::stack_member_rows(&app, 80, &theme);
    let summary = rows.first().unwrap();
    assert!(summary.line.to_string().contains("0 SKIP · 1 UNKNOWN"));
    assert_eq!(summary.line.spans[1].style.fg, Some(theme.muted));
    let row = rows
        .iter()
        .find(|row| row.line.to_string().trim_start().starts_with("UNKNOWN"))
        .unwrap();
    let span = row.line.spans.first().unwrap();

    assert_eq!(span.style.fg, Some(theme.muted));

    app.stack_inspector
        .checks
        .checks
        .first_mut()
        .unwrap()
        .status = PullRequestCheckStatus::Skipped;
    let (rows, _) = pull_request_stack_rows::stack_member_rows(&app, 80, &theme);
    assert_eq!(
        rows.first().unwrap().line.spans[1].style.fg,
        Some(theme.muted)
    );
}

#[test]
fn initial_member_error_does_not_claim_the_description_is_empty() {
    let mut app = stack_app();
    app.stack_inspector.selected_pull_request = None;
    app.stack_inspector.selected_error = Some("member metadata unavailable".to_owned());

    let (rows, _) = pull_request_stack_rows::stack_member_rows(&app, 80, &Theme::default());
    let output = rows
        .iter()
        .map(|row| row.line.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(output.contains("member metadata unavailable"));
    assert!(!output.contains("No description provided"));
}

#[test]
fn failed_stack_refresh_marks_last_known_topology_as_stale() {
    let mut app = stack_app();
    app.pull_request_stack_error = Some("stack metadata refresh failed".to_owned());
    app.stack_inspector.selected_error = Some("member metadata refresh failed".to_owned());
    app.pull_request_warnings
        .push("GitHub is unavailable; showing stale cached stack data".to_owned());
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let output = rendered(&terminal);
    assert!(output.contains("STACK #19"));
    assert!(output.contains("STALE METADATA"));
    assert!(output.contains("STACK GATE / TIP UNVERIFIED / STALE METADATA"));
    assert!(output.contains("BLOCKED"));
    assert!(output.contains("stack metadata refresh failed"));
    assert!(output.contains("member metadata refresh failed"));
    assert!(output.contains("Keeps permission refresh work bounded"));
    assert!(!output.contains("FINAL STACK GATE"));

    app.pull_request_stack_error = None;
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(rendered(&terminal).contains("WARN 1"));
}

#[test]
fn pull_request_stack_tab_is_hidden_without_stack_metadata() {
    let mut app = overview_app();
    let mut terminal = Terminal::new(TestBackend::new(120, 28)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    assert!(!app.geometry.sidebar_hits.iter().any(|hit| matches!(
        hit.target,
        SidebarHit::PullRequestStack | SidebarHit::PullRequestStackMember(_)
    )));
}
