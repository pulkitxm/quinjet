use super::*;

fn mouse(kind: MouseEventKind, column: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row: 10,
        modifiers: KeyModifiers::NONE,
    }
}

fn scroll_app() -> App {
    let mut app = app_with_changes();
    app.geometry.sidebar = Rect::new(0, 3, 30, 30);
    app.geometry.content = Rect::new(31, 3, 80, 30);
    app
}

#[test]
fn momentum_stops_when_the_pointer_crosses_panes_until_the_burst_ends() {
    let mut app = scroll_app();
    let now = Instant::now();
    drop(app.handle_mouse(mouse(MouseEventKind::ScrollDown, 50), now));
    drop(app.handle_mouse(
        mouse(MouseEventKind::Moved, 10),
        now + Duration::from_millis(10),
    ));
    for millis in [20, 50, 200, 400] {
        drop(app.handle_mouse(
            mouse(MouseEventKind::ScrollDown, 10),
            now + Duration::from_millis(millis),
        ));
    }
    assert_eq!(app.content_scroll, 2);
    assert_eq!(app.sidebar_offset, 0);
    assert_eq!(app.focus, Focus::Content);
    drop(app.handle_mouse(
        mouse(MouseEventKind::ScrollDown, 10),
        now + Duration::from_millis(700),
    ));
    assert_eq!(app.sidebar_offset, 2);
    assert_eq!(app.content_scroll, 2);
}

#[test]
fn wheel_coordinates_alone_cannot_transfer_a_scroll_burst() {
    let mut app = scroll_app();
    let now = Instant::now();
    drop(app.handle_mouse(mouse(MouseEventKind::ScrollDown, 10), now));
    drop(app.handle_mouse(
        mouse(MouseEventKind::ScrollDown, 50),
        now + Duration::from_millis(20),
    ));
    drop(app.handle_mouse(
        mouse(MouseEventKind::ScrollDown, 10),
        now + Duration::from_millis(30),
    ));
    assert_eq!(app.sidebar_offset, 2);
    assert_eq!(app.content_scroll, 0);
}

#[test]
fn moving_between_rows_in_one_pane_keeps_the_scroll_responsive() {
    let mut app = scroll_app();
    let now = Instant::now();
    drop(app.handle_mouse(mouse(MouseEventKind::ScrollDown, 50), now));
    drop(app.handle_mouse(mouse(MouseEventKind::Moved, 90), now));
    drop(app.handle_mouse(mouse(MouseEventKind::ScrollDown, 90), now));
    drop(app.handle_mouse(mouse(MouseEventKind::ScrollUp, 90), now));
    assert_eq!(app.content_scroll, 2);
    assert!(app.mouse_hover.is_none());
}

#[test]
fn headers_and_footers_do_not_scroll_any_pane() {
    let mut app = scroll_app();
    for row in [0, 1, 2, 33, 40] {
        drop(app.handle_mouse(
            MouseEvent {
                row,
                ..mouse(MouseEventKind::ScrollDown, 50)
            },
            Instant::now(),
        ));
    }
    assert_eq!(app.sidebar_offset, 0);
    assert_eq!(app.content_scroll, 0);
}

#[test]
fn momentum_cannot_scroll_a_newly_opened_modal() {
    let mut app = scroll_app();
    let now = Instant::now();
    drop(app.handle_mouse(mouse(MouseEventKind::ScrollDown, 50), now));
    app.modal = Some(Modal::CommandPalette {
        query: TextBuffer::default(),
        selected: 0,
    });
    app.geometry.modal_list_len = 20;
    app.geometry.modal_list_max_scroll = 10;
    drop(app.handle_mouse(
        mouse(MouseEventKind::ScrollDown, 50),
        now + Duration::from_millis(20),
    ));
    assert_eq!(app.modal_scroll, 0);
    drop(app.handle_mouse(
        mouse(MouseEventKind::ScrollDown, 50),
        now + Duration::from_millis(300),
    ));
    assert_eq!(app.modal_scroll, 2);
}

#[test]
fn horizontal_momentum_stops_after_leaving_code() {
    let mut app = scroll_app();
    app.horizontal_scroll_regions = vec![Rect::new(40, 4, 60, 20)];
    app.horizontal_scroll_limit = 100;
    let now = Instant::now();
    drop(app.handle_mouse(mouse(MouseEventKind::ScrollRight, 50), now));
    drop(app.handle_mouse(mouse(MouseEventKind::Moved, 10), now));
    drop(app.handle_mouse(mouse(MouseEventKind::ScrollRight, 50), now));
    assert_eq!(app.horizontal_scroll, 3);
}

#[test]
fn overscrolling_either_pane_does_not_delay_direction_reversal() {
    for column in [10, 50] {
        let mut app = scroll_app();
        app.geometry.sidebar_max_scroll = Some(8);
        app.geometry.content_max_scroll = Some(8);
        let now = Instant::now();
        for _ in 0..100 {
            drop(app.handle_mouse(mouse(MouseEventKind::ScrollDown, column), now));
        }
        drop(app.handle_mouse(mouse(MouseEventKind::ScrollUp, column), now));
        assert_eq!(
            if column == 10 {
                app.sidebar_offset
            } else {
                app.content_scroll
            },
            6
        );
    }
}
