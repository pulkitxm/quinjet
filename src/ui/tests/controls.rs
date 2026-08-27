use super::*;

#[test]
fn overflow_menus_right_align_to_the_cta_arrow() {
    let anchor = Rect::new(10, 20, 40, 1);
    let area = overflow_menu_area(anchor, 18, 4);
    assert_eq!(area.width, 18);
    assert_eq!(area.height, 6);
    assert_eq!(area.x, 32);
    assert_eq!(area.right(), anchor.right());
    assert_eq!(area.bottom(), anchor.y);
}

#[test]
fn confirm_modal_gives_yes_and_no_equal_hit_targets() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.modal = Some(Modal::Confirm {
        title: "Squash and merge?".to_owned(),
        message: "Really squash and merge #12 (Ship it)?".to_owned(),
        action: crate::app::ConfirmAction::Operate(crate::git::GitOperation::Fetch),
    });
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let yes = app
        .geometry
        .modal_action_hits
        .iter()
        .find(|(_, action)| *action == ModalAction::ConfirmYes)
        .map(|(area, _)| *area)
        .expect("yes button");
    let no = app
        .geometry
        .modal_action_hits
        .iter()
        .find(|(_, action)| *action == ModalAction::ConfirmNo)
        .map(|(area, _)| *area)
        .expect("no button");
    assert_eq!(yes.width, no.width);
    assert_eq!(yes.y, no.y);
    assert_eq!(yes.right(), no.x);
}

#[test]
fn the_toolbar_indent_is_shared_and_survives_a_label_that_cannot_fit() {
    assert_eq!(toolbar_indent(40, 12), 14);
    assert_eq!(toolbar_indent(40, 40), 0);
    assert_eq!(toolbar_indent(40, 64), 0);
    assert_eq!(toolbar_indent(13, 12), 1);
    assert_eq!(toolbar_indent(0, 6), 0);
}

#[test]
fn revert_and_stash_share_one_width_and_one_indent_at_any_count() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn toolbar(count: usize) -> (Rect, Rect, String, String, Buffer) {
        let theme = Theme::default();
        let mut app = App::new("/tmp/repo", "repo");
        app.status.changes = (0..count)
            .map(|index| Change {
                path: std::path::PathBuf::from(format!("file_{index}.txt")),
                original_path: None,
                area: ChangeArea::Unstaged,
                status: ChangeStatus::Modified,
            })
            .collect();
        app.checked_change_paths = app
            .status
            .changes
            .iter()
            .map(|change| change.path.clone())
            .collect();
        let mut terminal = Terminal::new(TestBackend::new(120, 34)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &theme))
            .unwrap();
        let area = |wanted: &ScmAction| {
            app.geometry
                .scm_action_hits
                .iter()
                .find(|hit| hit.action == *wanted)
                .map(|hit| hit.area)
                .expect("both toolbar buttons must be clickable")
        };
        let revert = area(&ScmAction::RevertChecked);
        let primary = area(&ScmAction::Primary);
        let buffer = terminal.backend().buffer().clone();
        let row = |area: Rect| {
            (area.x..area.right())
                .map(|x| buffer[(x, area.y)].symbol().to_owned())
                .collect::<String>()
        };
        (revert, primary, row(revert), row(primary), buffer)
    }

    for count in [1, 12, 128, 4096] {
        let (revert, primary, revert_row, primary_row, buffer) = toolbar(count);
        assert_eq!(revert.x, primary.x, "{count} files");
        assert_eq!(revert.width, primary.width, "{count} files");
        assert_eq!(revert.y.saturating_add(1), primary.y, "{count} files");
        assert_eq!(
            revert_row.len() - revert_row.trim_start().len(),
            primary_row.len() - primary_row.trim_start().len(),
            "{count} files indent"
        );
        assert!(revert_row.trim().starts_with(&format!("Revert ({count})")));
        assert!(primary_row.trim().starts_with(&format!("Stash ({count})")));
        let theme = Theme::default();
        assert_eq!(buffer[(revert.x, revert.y)].bg, theme.removed_background);
        assert_eq!(buffer[(primary.x, primary.y)].bg, theme.panel_alt);
    }
}

#[test]
fn help_catalog_covers_sections_and_previously_missing_bindings() {
    let sections = HELP_ROWS
        .iter()
        .filter_map(|row| match row {
            HelpRow::Section(title) => Some(*title),
            HelpRow::Shortcut { .. } | HelpRow::Spacer => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        sections,
        [
            "Navigation",
            "Changes",
            "Commits",
            "Pull Requests",
            "Check Logs",
            "Branches",
            "Stashes",
            "Conflict",
            "Repository",
        ]
    );

    let keys = HELP_ROWS
        .iter()
        .filter_map(|row| match row {
            HelpRow::Shortcut { keys, .. } => Some(*keys),
            HelpRow::Section(_) | HelpRow::Spacer => None,
        })
        .collect::<Vec<_>>();
    for expected in [
        "t / T",
        "m",
        "*",
        "Ctrl+D / Ctrl+U",
        "Space / ← / →",
        "gg / Home",
        "Ctrl+N",
        "Alt+A",
        "Option/Alt+1..9",
        "Ctrl+Delete",
        "Shift+P/F/S · Stack p/n, 1..5, r/d",
    ] {
        assert!(
            keys.contains(&expected),
            "help catalog should list {expected}"
        );
    }
    assert!(keys.contains(&"o"));

    assert_eq!(help_shortcut_count(false), keys.len());
    assert_eq!(help_display_index(0, false), 1);
    assert_eq!(help_shortcut_index_at(1, false), Some(0));
    assert_eq!(help_shortcut_index_at(0, false), None);

    let managed_keys = help_rows(true)
        .into_iter()
        .filter_map(|row| match row {
            HelpRow::Shortcut { keys, .. } => Some(*keys),
            HelpRow::Section(_) | HelpRow::Spacer => None,
        })
        .collect::<Vec<_>>();
    assert!(!managed_keys.contains(&"q"));
    assert!(!managed_keys.contains(&"Ctrl+W"));
    assert!(!managed_keys.contains(&"Right-click project tab"));
}

#[test]
fn help_modal_selects_rows_and_records_mouse_hits() {
    use std::time::Instant;

    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    let now = Instant::now();
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE), now);
    assert!(matches!(
        app.modal,
        Some(Modal::Help {
            selected: 0,
            hover: None,
            ..
        })
    ));

    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);
    assert!(matches!(
        app.modal,
        Some(Modal::Help {
            selected: 1,
            hover: None,
            ..
        })
    ));

    let backend = TestBackend::new(100, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(!app.geometry.help_hits.is_empty());
    let hit = app.geometry.help_hits[0].clone();

    let effects = app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Moved,
            column: hit.area.x,
            row: hit.area.y,
            modifiers: KeyModifiers::NONE,
        },
        now,
    );
    assert!(effects.is_empty());
    assert!(matches!(
        app.modal,
        Some(Modal::Help {
            hover: Some(index),
            ..
        }) if index == hit.index
    ));

    app.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: hit.area.x,
            row: hit.area.y,
            modifiers: KeyModifiers::NONE,
        },
        now,
    );
    assert!(matches!(
        app.modal,
        Some(Modal::Help {
            selected,
            ..
        }) if selected == hit.index
    ));

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(rendered.contains("Keyboard Shortcuts"));
    assert!(rendered.contains("Navigation"));
    assert!(rendered.contains("Changes"));
    assert!(rendered.contains("j/k select"));
    assert!(rendered.contains("Ctrl+D / Ctrl+U"));
    assert!(rendered.contains("t / T"));
}

#[test]
fn three_tabs_fit_the_minimum_supported_terminal_width() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.status.branch.head = "feature/a-very-long-branch-name".to_owned();
    let backend = TestBackend::new(72, 18);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    assert!(app.geometry.changes_tab.width > 0);
    assert!(app.geometry.history_tab.x > app.geometry.changes_tab.x);
    assert!(app.geometry.pull_requests_tab.x > app.geometry.history_tab.x);
    assert!(app.geometry.pull_requests_tab.right() <= 72);
}

#[test]
fn pull_request_section_tabs_distinguish_navigation_from_row_selection() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let theme = Theme::default();
    let mut terminal = Terminal::new(TestBackend::new(20, 1)).unwrap();

    terminal
        .draw(|frame| {
            draw_pull_request_section_tab(
                frame,
                Rect::new(0, 0, 10, 1),
                "PR".to_owned(),
                true,
                &theme,
            );
            draw_pull_request_section_tab(
                frame,
                Rect::new(10, 0, 10, 1),
                "Files".to_owned(),
                false,
                &theme,
            );
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(5, 0)].bg, theme.accent_soft);
    assert_eq!(buffer[(5, 0)].fg, theme.text);
    assert_eq!(buffer[(15, 0)].bg, theme.panel);
    assert_eq!(buffer[(15, 0)].fg, theme.muted);
    assert_ne!(buffer[(5, 0)].bg, theme.selected);
}

#[test]
fn pane_frames_keep_titles_without_vertical_box_lines() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let theme = Theme::default();
    let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(
                panel_block(" Preview ".to_owned(), true, &theme),
                frame.area(),
            );
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(0, 1)].symbol(), " ");
    assert_eq!(buffer[(19, 1)].symbol(), " ");
    assert!(buffer.content().iter().all(|cell| cell.symbol() != "│"));
}

#[test]
fn a_light_theme_reaches_the_entire_render_surface() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    let theme = Theme::new(ThemeName::TokyoNight, crate::theme::Appearance::Light);
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    let buffer = terminal.backend().buffer();

    assert_eq!(buffer[(99, 23)].style().bg, Some(theme.panel_alt));
    assert!(
        buffer
            .content()
            .iter()
            .any(|cell| cell.style().bg == Some(theme.background))
    );
    assert!(
        buffer
            .content()
            .iter()
            .any(|cell| cell.style().fg == Some(theme.text))
    );
    assert!(
        buffer
            .content()
            .iter()
            .any(|cell| cell.style().fg == Some(theme.accent))
    );
}

#[test]
fn theme_picker_shows_every_family_and_marks_the_current_one() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.set_theme_selection(ThemeName::TokyoNight, AppearanceChoice::Dark);
    app.modal = Some(Modal::Themes {
        selected: 9,
        original: ThemeName::TokyoNight,
    });
    let theme = app.theme;
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();

    assert!(rendered.contains("Select Theme"));
    assert!(rendered.contains("Catppuccin"));
    assert!(rendered.contains("✓ Tokyo Night"));
    assert!(rendered.contains("GitHub"));
    assert!(rendered.contains("Enter apply   Esc close"));
}

#[test]
fn theme_picker_keeps_the_last_theme_visible_at_minimum_height() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.set_theme_selection(ThemeName::Github, AppearanceChoice::Dark);
    app.modal = Some(Modal::Themes {
        selected: 12,
        original: ThemeName::Github,
    });
    let theme = app.theme;
    let mut terminal = Terminal::new(TestBackend::new(80, 18)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &theme))
        .unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();

    assert!(rendered.contains("✓ GitHub"));
    assert!(rendered.contains("Enter apply   Esc close"));
}
