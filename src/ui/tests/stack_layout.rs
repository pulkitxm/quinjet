use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::stack::{rendered, stack_app};
use super::*;

fn row_text(terminal: &Terminal<TestBackend>, row: u16, width: u16) -> String {
    let buffer = terminal.backend().buffer();
    (0..width)
        .map(|column| buffer[(column, row)].symbol())
        .collect()
}

#[test]
fn wide_stack_regions_have_titled_separators_without_changing_hit_rows() {
    let mut app = stack_app();
    let theme = Theme::default();
    let mut terminal = Terminal::new(TestBackend::new(104, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();

    let gate = app
        .geometry
        .stack_inspector_hits
        .iter()
        .find(|hit| hit.target == StackInspectorHit::TipChecks)
        .unwrap()
        .area;
    let gate_title = row_text(&terminal, gate.y, 104);
    let gate_state = row_text(&terminal, gate.y.saturating_add(1), 104);
    let rail_title = row_text(&terminal, app.geometry.sidebar.y, 104);
    let detail_title = row_text(&terminal, app.geometry.content.y, 104);
    let buffer = terminal.backend().buffer();

    assert!(gate_title.contains("FINAL GATE FAIL"));
    assert!(gate_state.contains("[t Inspect tip checks]"));
    assert!(rail_title.contains("REVIEW PATH · BASE TO TIP"));
    assert_eq!(
        buffer[(
            app.geometry.sidebar.right().saturating_sub(1),
            app.geometry.sidebar.y
        )]
            .symbol(),
        "─"
    );
    assert!(detail_title.contains("Member 3/3"));
    assert_eq!(app.geometry.sidebar_divider.width, 1);
    assert!(
        app.geometry
            .sidebar_hits
            .iter()
            .all(|hit| hit.area.y >= app.geometry.sidebar.y.saturating_add(1))
    );
}

#[test]
fn compact_stack_regions_keep_their_height_and_gain_separators() {
    let mut app = stack_app();
    let theme = Theme::default();
    let mut terminal = Terminal::new(TestBackend::new(72, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();

    let gate = app
        .geometry
        .stack_inspector_hits
        .iter()
        .find(|hit| hit.target == StackInspectorHit::TipChecks)
        .unwrap()
        .area;
    let gate_title = row_text(&terminal, gate.y, 72);
    let member_title = row_text(&terminal, app.geometry.sidebar.y, 72);
    let detail_title = row_text(&terminal, app.geometry.content.y, 72);
    let output = rendered(&terminal);

    assert!(gate_title.contains("FINAL GATE FAIL"));
    assert!(member_title.contains("REVIEW PATH · p/n select"));
    assert!(detail_title.contains("Member 3/3"));
    assert_eq!(app.geometry.sidebar.height, 2);
    assert_eq!(app.geometry.sidebar_divider, Rect::default());
    assert!(output.contains("[t Inspect tip checks]"));
    assert!(!gate_title.contains(['┌', '┐', '│']));
    assert!(!member_title.contains(['┌', '┐', '│']));
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
