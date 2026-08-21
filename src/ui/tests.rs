
use super::*;
use crate::git::diff::CommitDetails;

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
        "Ctrl+Delete",
    ] {
        assert!(
            keys.contains(&expected),
            "help catalog should list {expected}"
        );
    }
    assert!(keys.contains(&"o"));

    assert_eq!(help_shortcut_count(), keys.len());
    assert_eq!(help_display_index(0), 1);
    assert_eq!(help_shortcut_index_at(1), Some(0));
    assert_eq!(help_shortcut_index_at(0), None);
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

#[test]
fn header_registers_repository_branch_and_workspace_links() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.status.branch.head = "feature/link".to_owned();
    app.local_github_repository = Some(GitHubRepository {
        name_with_owner: "acme/repo".to_owned(),
        url: "https://github.com/acme/repo".to_owned(),
        remotes: vec!["origin".to_owned()],
    });
    let backend = TestBackend::new(160, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    assert!(app.geometry.link_hits.iter().any(|hit| matches!(
        &hit.target,
        OpenTarget::Browser(url) if url == "https://github.com/acme/repo"
    )));
    assert!(app.geometry.link_hits.iter().any(|hit| matches!(
        &hit.target,
        OpenTarget::Browser(url)
            if url == "https://github.com/acme/repo/tree/feature/link"
    )));
    assert!(
        app.geometry
            .project_hits
            .iter()
            .any(|area| area.width > 0 && area.height > 0)
    );
    let repository_area = app
        .geometry
        .link_hits
        .iter()
        .find(|hit| {
            matches!(
                &hit.target,
                OpenTarget::Browser(url) if url == "https://github.com/acme/repo"
            )
        })
        .map(|hit| hit.area)
        .unwrap();
    assert!(
        !terminal.backend().buffer()[(repository_area.x, repository_area.y)]
            .modifier
            .contains(Modifier::UNDERLINED)
    );
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(!rendered.contains("\x1b]8;;"));

    app.link_hover = Some((repository_area.x, repository_area.y));
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(
        terminal.backend().buffer()[(repository_area.x, repository_area.y)]
            .modifier
            .contains(Modifier::UNDERLINED)
    );

    app.configure_mouse_capture(false);
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(rendered.contains("\x1b]8;;https://github.com/acme/repo\x1b\\"));
    assert!(rendered.contains("\x1b]8;;https://github.com/acme/repo/tree/feature/link\x1b\\"));

    app.view = View::History;
    app.link_hover = None;
    app.history = vec![crate::git::history::Commit {
        id: "abc123".to_owned(),
        short_id: "abc123".to_owned(),
        parent_ids: Vec::new(),
        author: String::new(),
        author_email: String::new(),
        authored_at: String::new(),
        committer: String::new(),
        committer_email: String::new(),
        committed_at: String::new(),
        relative_date: "now".to_owned(),
        subject: "Linked commit".to_owned(),
        decorations: Vec::new(),
    }];
    terminal.clear().unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(rendered.contains("\x1b]8;;https://github.com/acme/repo/commit/abc123\x1b\\"));
}

#[test]
fn footer_underlines_only_the_worktree_count() {
    use std::path::PathBuf;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.status.branch.head = "main".to_owned();
    let tree = |path: &str, current: bool| crate::git::Worktree {
        path: PathBuf::from(path),
        head: "abcdef0123456789".to_owned(),
        branch: Some("main".to_owned()),
        current,
        bare: false,
        detached: false,
        locked: None,
        prunable: None,
    };
    app.worktrees = vec![
        tree("/tmp/repo", true),
        tree("/tmp/repo-a", false),
        tree("/tmp/repo-b", false),
    ];
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let hit = app
        .geometry
        .project_hits
        .iter()
        .copied()
        .max_by_key(|area| area.y)
        .expect("footer worktree hit");
    let buffer = terminal.backend().buffer();
    let mut label = String::new();
    for x in hit.x..hit.right() {
        let cell = &buffer[(x, hit.y)];
        label.push_str(cell.symbol());
        assert!(
            cell.modifier.contains(Modifier::UNDERLINED),
            "worktree label should be underlined"
        );
    }
    assert_eq!(label, "3 worktrees");
    let before = &buffer[(hit.x.saturating_sub(1), hit.y)];
    assert_eq!(before.symbol(), " ");
    assert!(!before.modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn preview_selection_highlights_only_the_pane_where_it_started() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let theme = Theme::default();
    let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
    let selection = crate::app::TextSelection {
        pane: Rect::new(10, 0, 10, 4),
        anchor: (12, 1),
        head: (18, 2),
    };

    terminal
        .draw(|frame| draw_text_selection(frame, selection, &theme))
        .unwrap();

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(12, 1)].bg, theme.selected);
    assert_eq!(buffer[(18, 2)].bg, theme.selected);
    assert_ne!(buffer[(9, 1)].bg, theme.selected);
    assert_ne!(buffer[(9, 2)].bg, theme.selected);
}

#[test]
fn changes_view_exposes_vscode_style_file_group_and_toolbar_actions() {
    use std::path::PathBuf;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.status.changes = vec![
        Change {
            path: PathBuf::from("src/main.rs"),
            original_path: None,
            area: ChangeArea::Unstaged,
            status: ChangeStatus::Modified,
        },
        Change {
            path: PathBuf::from("README.md"),
            original_path: None,
            area: ChangeArea::Staged,
            status: ChangeStatus::Modified,
        },
        Change {
            path: PathBuf::from("notes.txt"),
            original_path: None,
            area: ChangeArea::Unstaged,
            status: ChangeStatus::Untracked,
        },
    ];
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

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

    assert!(rendered.contains("[+]"));
    assert!(rendered.contains("[−]"));
    assert!(rendered.contains("Commit"));
    assert!(rendered.contains('▶'));
    assert!(rendered.contains("[ ]"));
    assert!(!rendered.contains("[c] Commit"));
    assert!(!rendered.contains("[S] Stashes"));
    assert!(!rendered.contains("[d] Compare Branch"));
    assert!(!rendered.contains("UNTRACKED CHANGES"));
    assert!(rendered.contains("CHANGES"));
    assert!(rendered.contains("notes.txt"));
    assert!(rendered.contains("\u{e7a8} main.rs"));
    assert!(rendered.contains("\u{eeab} README.md"));
    assert!(!rendered.contains('›'));
    assert!(
        app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| matches!(hit.action, ScmAction::Stage(0)))
    );
    assert!(
        app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| matches!(hit.action, ScmAction::Unstage(1)))
    );
    assert!(
        app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| matches!(hit.action, ScmAction::ToggleCheck(_)))
    );
    assert!(
        app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| matches!(hit.action, ScmAction::Primary))
    );
}

#[test]
fn middle_truncation_respects_display_width() {
    let result = truncate_middle("src/a-very-long-file-name.rs", 14);
    assert!(result.width() <= 14);
    assert!(result.contains('…'));
    assert!(result.ends_with("me.rs"));
}

#[test]
fn a_scrollable_content_pane_offers_a_jump_to_bottom_control() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.set_document(DiffDocument {
        title: "Changes".to_owned(),
        truncated: false,
        commit_details: None,
        pull_request_details: None,
        lines: (0..200)
            .map(|index| test_line(DiffLineKind::Context, &format!("line {index}")))
            .collect(),
    });
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(
        app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| hit.action == ScmAction::JumpToBottom),
        "a long document offers the control"
    );

    app.content_scroll = usize::MAX;
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(
        !app.geometry
            .scm_action_hits
            .iter()
            .any(|hit| hit.action == ScmAction::JumpToBottom),
        "the control disappears once the reader is at the bottom"
    );
}

#[test]
fn diff_rows_are_cached_between_draws_and_rebuilt_on_document_change() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.set_document(DiffDocument {
        title: "Changes".to_owned(),
        truncated: false,
        commit_details: None,
        pull_request_details: None,
        lines: vec![
            test_file_header("src/main.rs", 1, 0),
            test_line(DiffLineKind::Context, "same"),
            test_line(DiffLineKind::FileFooter, ""),
        ],
    });
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(
        !app.unified_diff_rows.is_empty(),
        "the first draw builds the unified rows"
    );
    let key = app.diff_rows_key;
    let pointer = app.unified_diff_rows.as_ptr();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert_eq!(
        app.diff_rows_key, key,
        "an unchanged document keeps its key"
    );
    assert_eq!(
        app.unified_diff_rows.as_ptr(),
        pointer,
        "an unchanged document reuses its rows"
    );

    app.set_document(DiffDocument::empty("Changes", "Working tree clean"));
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert_ne!(
        app.diff_rows_key, key,
        "replacing the document rebuilds the rows"
    );
    assert_eq!(
        app.unified_diff_rows.len(),
        app.document.lines.len(),
        "the rebuilt rows describe the new document"
    );
}

#[test]
fn side_by_side_pairs_replacements() {
    let document = DiffDocument {
        title: String::new(),
        truncated: false,
        commit_details: None,
        pull_request_details: None,
        lines: vec![
            test_line(DiffLineKind::Removed, "old one"),
            test_line(DiffLineKind::Removed, "old two"),
            test_line(DiffLineKind::Added, "new one"),
            test_line(DiffLineKind::Context, "same"),
        ],
    };
    let app = App::new("/tmp/repo", "repo");
    let rows = side_by_side_rows(&document, &app);
    assert_eq!(rows.len(), 3);
    let line_text = |index: usize| document.lines.get(index).map(DiffLine::text).unwrap();
    let SideBySideRow::Split(old, new) = &rows[0] else {
        panic!("expected a split diff row");
    };
    assert_eq!(line_text(old.unwrap()), "old one");
    assert_eq!(line_text(new.unwrap()), "new one");
    let SideBySideRow::Split(_, new) = &rows[1] else {
        panic!("expected a split diff row");
    };
    assert!(new.is_none());
    let SideBySideRow::Split(old, _) = &rows[2] else {
        panic!("expected a split diff row");
    };
    assert_eq!(line_text(old.unwrap()), "same");
}

#[test]
fn pull_request_folders_render_as_clickable_collapse_controls() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_files = ["src/app.rs", "src/git/diff.rs"]
        .into_iter()
        .map(|path| PullRequestFile {
            path: std::path::PathBuf::from(path),
            old_path: None,
            status: PullRequestFileStatus::Modified,
            counts: None,
        })
        .collect();
    let backend = TestBackend::new(40, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut hits = Vec::new();

    terminal
        .draw(|frame| {
            hits = draw_pull_request_file_tree(frame, frame.area(), &mut app, &Theme::default());
        })
        .unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(rendered.contains("⌄ src/"));
    assert!(rendered.contains("\u{e7a8} app.rs"));
    assert!(rendered.contains("app.rs"));
    assert!(hits.iter().any(|hit| {
        matches!(
            &hit.target,
            SidebarHit::PullRequestDirectory(path) if path == Path::new("src")
        )
    }));

    app.collapsed_pull_request_directories
        .insert(std::path::PathBuf::from("src"));
    app.pull_request_tree.clear();
    terminal.clear().unwrap();
    terminal
        .draw(|frame| {
            draw_pull_request_file_tree(frame, frame.area(), &mut app, &Theme::default());
        })
        .unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(rendered.contains("› src/"));
    assert!(!rendered.contains("app.rs"));
}

#[test]
fn pull_request_file_tree_virtualizes_a_thousand_files() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request_files = (0..1_000)
        .map(|index| PullRequestFile {
            path: std::path::PathBuf::from(format!(
                "packages/package-{index:04}/src/file-{index:04}.rs"
            )),
            old_path: None,
            status: PullRequestFileStatus::Modified,
            counts: None,
        })
        .collect();
    app.pull_request_total_files = app.pull_request_files.len();
    app.pull_request_file_cursor = 999;
    let rows = app.pull_request_tree_entries();
    assert_eq!(
        rows.iter()
            .filter(|row| matches!(row, PullRequestTreeEntry::File { .. }))
            .count(),
        1_000
    );
    app.pull_request_tree_cursor = rows
        .iter()
        .position(|row| matches!(row, PullRequestTreeEntry::File { index: 999, .. }))
        .unwrap();

    let backend = TestBackend::new(48, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_pull_request_file_tree(frame, frame.area(), &mut app, &Theme::default());
        })
        .unwrap();
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();

    assert!(app.sidebar_offset > 0);
    assert!(rendered.contains("file-0999.rs"));
}

#[test]
fn hides_raw_hunk_coordinates_in_both_diff_layouts() {
    let document = DiffDocument {
        title: String::new(),
        truncated: false,
        commit_details: None,
        pull_request_details: None,
        lines: vec![
            test_file_header("src/main.rs", 1, 0),
            test_line(DiffLineKind::HunkHeader, "@@ -10,2 +10,3 @@ fn main()"),
            test_line(DiffLineKind::Context, "same"),
            test_line(DiffLineKind::FileFooter, ""),
        ],
    };

    let app = App::new("/tmp/repo", "repo");
    assert_eq!(unified_row_indices(&document, &app), vec![0, 2, 3]);
    assert!(side_by_side_rows(&document, &app).iter().all(|row| {
        !matches!(
            row,
            SideBySideRow::Full { index, .. }
                if document
                    .lines
                    .get(*index)
                    .is_some_and(|line| line.kind == DiffLineKind::HunkHeader)
        )
    }));
}

#[test]
fn commit_preview_renders_details_once_and_names_each_file_pane() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::History;
    app.focus = Focus::Content;
    app.status.branch.head = "main".to_owned();
    app.history_branch = Some(HistoryBranch {
        name: "origin/topic".to_owned(),
        reference: "refs/remotes/origin/topic".to_owned(),
        current: false,
        remote: true,
        relative_date: "now".to_owned(),
        short_id: "abc1234".to_owned(),
    });
    app.document = DiffDocument {
        title: "abc1234 — Improve history".to_owned(),
        truncated: false,
        commit_details: Some(CommitDetails {
            id: "abc123456789".to_owned(),
            subject: "Improve history".to_owned(),
            author: "Ada".to_owned(),
            author_email: "ada@example.com".to_owned(),
            authored_at: "2026-01-02T03:04:05Z".to_owned(),
            committer: "Grace".to_owned(),
            committer_email: "grace@example.com".to_owned(),
            committed_at: "2026-01-02T04:05:06Z".to_owned(),
        }),
        pull_request_details: None,
        lines: vec![
            test_file_header("src/main.rs", 1, 0),
            test_line(DiffLineKind::HunkHeader, "@@ -1,0 +1 @@"),
            test_line(DiffLineKind::Added, "fn main() {}"),
            test_line(DiffLineKind::FileFooter, ""),
            test_file_header("README.md", 1, 0),
            test_line(DiffLineKind::HunkHeader, "@@ -1,0 +1 @@"),
            test_line(DiffLineKind::Added, "# Quinjet"),
            test_line(DiffLineKind::FileFooter, ""),
        ],
    };
    let backend = TestBackend::new(140, 32);
    let mut terminal = Terminal::new(backend).unwrap();
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

    assert_eq!(rendered.matches("Commit details").count(), 1);
    assert!(rendered.contains("origin/topic"));
    assert!(rendered.contains("[b branch]"));
    assert!(rendered.contains("src/main.rs"));
    assert!(rendered.contains("README.md"));
    assert!(!rendered.contains("@@"));
    assert!(!rendered.contains('◆'));
    assert!(!rendered.contains('░'));
}

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
#[test]
fn pull_request_preview_renders_cross_remote_metadata_and_diff() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.focus = Focus::Content;
    app.pull_request_section = PullRequestSection::Files;
    app.pull_request_exact_number = Some(42);
    app.pull_request_lookup = crate::app::TextBuffer::new("42");
    app.pull_request = Some(crate::git::github::PullRequest {
        number: 42,
        title: "Ship the rocket".to_owned(),
        description:
            "## Summary\n- Launch **safely** after all checks pass\n- Keep raw `gh` output bounded"
                .to_owned(),
        author: "octocat".to_owned(),
        state: "OPEN".to_owned(),
        is_draft: false,
        created_at: String::new(),
        updated_at: String::new(),
        url: "https://github.com/acme/widget/pull/42".to_owned(),
        base_ref: "main".to_owned(),
        base_oid: String::new(),
        head_ref: "feature/rocket".to_owned(),
        head_oid: String::new(),
        base_repository: GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://github.com/acme/widget".to_owned(),
            remotes: vec!["upstream".to_owned()],
        },
        head_repository: Some("octocat/widget".to_owned()),
        head_remotes: vec!["origin".to_owned(), "publish".to_owned()],
        is_cross_repository: true,
        additions: 101,
        deletions: 20,
        changed_files: 1,
    });
    app.pull_request_repository = Some(GitHubRepository {
        name_with_owner: "acme/widget".to_owned(),
        url: "https://github.com/acme/widget".to_owned(),
        remotes: vec!["upstream".to_owned()],
    });
    app.pull_request_files = vec![PullRequestFile {
        path: std::path::PathBuf::from("src/rocket.rs"),
        old_path: None,
        status: PullRequestFileStatus::Added,
        counts: None,
    }];
    app.pull_request_total_files = 1;
    app.document = DiffDocument {
        title: "PR #42 — Ship the rocket".to_owned(),
        truncated: false,
        commit_details: None,
        pull_request_details: Some(PullRequestDetails {
            number: 42,
            title: "Ship the rocket".to_owned(),
            description: "Launch safely after all checks pass".to_owned(),
            author: "octocat".to_owned(),
            state: "OPEN".to_owned(),
            is_draft: false,
            updated_at: "2026-08-13T12:00:00Z".to_owned(),
            url: "https://github.com/acme/widget/pull/42".to_owned(),
            base_repository: "acme/widget".to_owned(),
            base_ref: "main".to_owned(),
            base_remotes: vec!["upstream".to_owned()],
            head_repository: Some("octocat/widget".to_owned()),
            head_ref: "feature/rocket".to_owned(),
            head_remotes: vec!["origin".to_owned(), "publish".to_owned()],
            is_cross_repository: true,
            changed_files: 1,
            additions: 101,
            deletions: 20,
            selected_file: Some("src/rocket.rs".to_owned()),
            selected_file_additions: 1,
            selected_file_deletions: 0,
        }),
        lines: vec![
            test_file_header("src/rocket.rs", 1, 0),
            test_line(DiffLineKind::HunkHeader, "@@ -0,0 +1 @@"),
            test_line(DiffLineKind::Added, "launch();"),
            test_line(DiffLineKind::FileFooter, ""),
        ],
    };
    let backend = TestBackend::new(160, 34);
    let mut terminal = Terminal::new(backend).unwrap();

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

    assert!(rendered.contains("Pull Requests"));
    assert!(rendered.contains("Files 1"));
    assert!(rendered.contains("PR"));
    assert!(rendered.contains("rocket.rs"));
    assert!(rendered.contains("launch();"));
    assert!(!rendered.contains("Page"));
    assert!(!rendered.contains("files on page"));
    assert!(!rendered.contains("@@"));
    for expected in [
        "https://github.com/acme/widget",
        "https://github.com/acme/widget/pull/42",
        "https://github.com/octocat",
        "https://github.com/octocat/widget/tree/feature/rocket",
        "https://github.com/acme/widget/tree/main",
    ] {
        assert!(
            app.geometry
                .link_hits
                .iter()
                .any(|hit| { matches!(&hit.target, OpenTarget::Browser(url) if url == expected) }),
            "missing link target {expected}"
        );
    }

    app.pull_request_section = PullRequestSection::Overview;
    app.pull_request_checks = vec![PullRequestCheck {
        name: "CI / ubuntu".to_owned(),
        workflow: "CI".to_owned(),
        state: "SUCCESS".to_owned(),
        status: PullRequestCheckStatus::Passed,
        description: "All jobs passed".to_owned(),
        link: "https://github.com/acme/widget/actions/1".to_owned(),
        started_at: "2026-08-13T12:00:00Z".to_owned(),
        completed_at: "2026-08-13T12:01:00Z".to_owned(),
    }];
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
    assert!(rendered.contains("Conversation"));
    assert!(!rendered.contains(['⟳', '↻', '↺']));
    assert!(rendered.contains("CI / ubuntu"));
    assert!(rendered.contains("Ship the rocket"));
    assert!(rendered.contains("octocat/widget:feature/rocket"));
    assert!(rendered.contains("acme/widget:main"));
    assert!(rendered.contains("+101"));
    assert!(rendered.contains("-20"));
    assert!(
        rendered.contains("Launch"),
        "the pull-request body is part of the default view"
    );

    app.pull_request_conversation_loading = true;
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
    assert!(rendered.contains("loading"));
    assert!(!rendered.contains(['⟳', '↻', '↺']));
}

fn overview_app() -> App {
    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_exact_number = Some(42);
    app.pull_request = Some(crate::git::github::PullRequest {
        number: 42,
        title: "Ship the rocket".to_owned(),
        description: "## Summary\n- Launch **safely**\n\n```sh\ncargo test\n```".to_owned(),
        author: "octocat".to_owned(),
        state: "OPEN".to_owned(),
        is_draft: false,
        created_at: "2026-08-01T09:00:00Z".to_owned(),
        updated_at: "2026-08-02T10:30:00Z".to_owned(),
        url: "https://github.com/acme/widget/pull/42".to_owned(),
        base_ref: "main".to_owned(),
        base_oid: String::new(),
        head_ref: "feature/rocket".to_owned(),
        head_oid: String::new(),
        base_repository: GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://github.com/acme/widget".to_owned(),
            remotes: vec!["origin".to_owned()],
        },
        head_repository: Some("acme/widget".to_owned()),
        head_remotes: vec!["origin".to_owned()],
        is_cross_repository: false,
        additions: 101,
        deletions: 20,
        changed_files: 3,
    });
    app.pull_request_checks = vec![PullRequestCheck {
        name: "Format, lint, and test".to_owned(),
        workflow: "CI".to_owned(),
        state: "FAILURE".to_owned(),
        status: PullRequestCheckStatus::Failed,
        description: String::new(),
        link: "https://github.com/acme/widget/actions/runs/9/job/12".to_owned(),
        started_at: "2026-08-02T10:00:00Z".to_owned(),
        completed_at: "2026-08-02T10:02:30Z".to_owned(),
    }];
    app
}

#[test]
fn pull_request_and_check_urls_share_clickable_link_hits() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    let conversation_url = "https://github.com/acme/widget/pull/42#issuecomment-123".to_owned();
    app.pull_request_conversation.entries = vec![ConversationEntry {
        kind: ConversationKind::Comment,
        actor: "reviewer".to_owned(),
        timestamp: "2026-08-02T11:00:00Z".to_owned(),
        detail: String::new(),
        body: "Looks good".to_owned(),
        url: conversation_url.clone(),
        reference: String::new(),
        context: String::new(),
    }];
    let mut terminal = Terminal::new(TestBackend::new(140, 32)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    let pull_request_url = "https://github.com/acme/widget/pull/42";
    assert_eq!(
        app.geometry
            .link_hits
            .iter()
            .filter(|hit| matches!(
                &hit.target,
                OpenTarget::Browser(url) if url == pull_request_url
            ))
            .count(),
        3
    );
    for expected in [
        "https://github.com/octocat",
        "https://github.com/reviewer",
        "https://github.com/acme/widget/tree/feature/rocket",
        "https://github.com/acme/widget/tree/main",
        conversation_url.as_str(),
    ] {
        assert!(
            app.geometry
                .link_hits
                .iter()
                .any(|hit| { matches!(&hit.target, OpenTarget::Browser(url) if url == expected) })
        );
    }
    let pull_request_url_area = app
        .geometry
        .link_hits
        .iter()
        .filter(|hit| {
            matches!(
                &hit.target,
                OpenTarget::Browser(url) if url == pull_request_url
            )
        })
        .max_by_key(|hit| hit.area.width)
        .map(|hit| hit.area)
        .unwrap();
    let effects = app.handle_mouse(
        crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: pull_request_url_area.x,
            row: pull_request_url_area.y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
        std::time::Instant::now(),
    );
    assert!(matches!(
        effects.as_slice(),
        [crate::app::AppEffect::Open(OpenTarget::Browser(url))]
            if url == pull_request_url
    ));

    app.pull_request_check_cursor = Some(0);
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let check_url = "https://github.com/acme/widget/actions/runs/9/job/12";
    assert_eq!(
        app.geometry
            .link_hits
            .iter()
            .filter(|hit| matches!(
                &hit.target,
                OpenTarget::Browser(url) if url == check_url
            ))
            .count(),
        2
    );
    let check_url_area = app
        .geometry
        .link_hits
        .iter()
        .filter(|hit| {
            matches!(
                &hit.target,
                OpenTarget::Browser(url) if url == check_url
            )
        })
        .max_by_key(|hit| hit.area.width)
        .map(|hit| hit.area)
        .unwrap();
    let effects = app.handle_mouse(
        crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: check_url_area.x,
            row: check_url_area.y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        },
        std::time::Instant::now(),
    );
    assert!(matches!(
        effects.as_slice(),
        [crate::app::AppEffect::Open(OpenTarget::Browser(url))] if url == check_url
    ));
}

#[test]
fn a_long_conversation_stays_bounded_to_render() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    let body = "Some reasonably long review comment body that wraps across several \
terminal rows because that is what real pull-request comments look like in practice."
        .to_owned();
    app.pull_request_conversation = crate::git::github::PullRequestConversation {
        truncated: true,
        from_cache: false,
        entries: (0..500)
            .map(|index| ConversationEntry {
                kind: if index % 3 == 0 {
                    ConversationKind::Comment
                } else {
                    ConversationKind::Commit
                },
                actor: "octocat".to_owned(),
                timestamp: "2026-08-02T09:10:00Z".to_owned(),
                detail: "abc1234".to_owned(),
                body: body.clone(),
                url: String::new(),
                reference: String::new(),
                context: String::new(),
            })
            .collect(),
    };

    let rows = conversation_rows(&app, 120, &Theme::default());

    assert!(
        rows.len() < 3_000,
        "a thread at the fetch cap still builds a bounded number of rows: {}",
        rows.len()
    );
    assert!(
        rows.iter()
            .any(|row| row.line.to_string().contains("Older activity was omitted")),
        "a truncated thread says so rather than silently dropping history"
    );

    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let cache_key = app.pull_request_content_rows_key;
    let cache_pointer = app.pull_request_content_rows.as_ptr();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert_eq!(app.pull_request_content_rows_key, cache_key);
    assert_eq!(app.pull_request_content_rows.as_ptr(), cache_pointer);
}

#[test]
fn a_large_check_log_scrolls_from_a_cached_layout() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    app.pull_request_check_cursor = Some(0);
    assert!(app.expanded_check_steps.insert(1));
    app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
        steps: vec![CheckStep {
            number: 1,
            name: "Large build".to_owned(),
            status: PullRequestCheckStatus::Passed,
            conclusion: "success".to_owned(),
            started_at: String::new(),
            completed_at: String::new(),
            lines: (0..50_000)
                .map(|index| CheckLogLine {
                    timestamp: String::new(),
                    text: format!("output line {index}"),
                    severity: CheckLogSeverity::Normal,
                })
                .collect(),
        }],
        loose_lines: Vec::new(),
        truncated: false,
        unavailable: None,
        log_pending: false,
    });
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert!(app.pull_request_content_rows.len() > 50_000);
    let cache_pointer = app.pull_request_content_rows.as_ptr();
    app.content_scroll = usize::MAX;
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

    assert_eq!(app.pull_request_content_rows.as_ptr(), cache_pointer);
    assert!(rendered.contains("output line 49999"));
}

#[test]
fn an_expanded_step_can_be_scrolled_past_to_reach_the_steps_below_it() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    app.pull_request_check_cursor = Some(0);
    let line = |text: &str| CheckLogLine {
        timestamp: "2026-08-02T10:00:01Z".to_owned(),
        text: text.to_owned(),
        severity: CheckLogSeverity::Normal,
    };
    let step = |number: usize, lines: usize| CheckStep {
        number,
        name: format!("Step {number}"),
        status: PullRequestCheckStatus::Passed,
        conclusion: "success".to_owned(),
        started_at: "2026-08-02T10:00:00Z".to_owned(),
        completed_at: "2026-08-02T10:00:05Z".to_owned(),
        lines: (0..lines)
            .map(|index| line(&format!("output line {index}")))
            .collect(),
    };
    app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
        steps: vec![step(1, 300), step(2, 0), step(3, 0)],
        loose_lines: Vec::new(),
        truncated: false,
        unavailable: None,
        log_pending: false,
    });
    app.expanded_check_steps.insert(1);
    app.pull_request_step_cursor = 1;
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();

    app.content_scroll = 120;
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert_eq!(
        app.content_scroll, 120,
        "a redraw must not drag the view back to the selected step"
    );

    app.content_scroll = usize::MAX;
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
    assert!(
        rendered.contains("Step 2") && rendered.contains("Step 3"),
        "the steps below a long expanded step are reachable by scrolling"
    );
    assert!(
        !rendered.contains("output line 0 "),
        "the view really moved past the expanded output"
    );
}

#[test]
fn the_selected_step_is_the_row_that_is_highlighted() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    let theme = Theme::default();
    app.pull_request_check_cursor = Some(0);
    let step = |number: usize| CheckStep {
        number,
        name: format!("Run step {number}"),
        status: PullRequestCheckStatus::Passed,
        conclusion: "success".to_owned(),
        started_at: "2026-08-02T10:00:00Z".to_owned(),
        completed_at: "2026-08-02T10:00:01Z".to_owned(),
        lines: Vec::new(),
    };
    app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
        steps: (1..=4).map(step).collect(),
        loose_lines: Vec::new(),
        truncated: false,
        unavailable: None,
        log_pending: false,
    });
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();

    let highlighted = |app: &mut App, terminal: &mut Terminal<TestBackend>| {
        terminal.draw(|frame| draw(frame, app, &theme)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..30)
            .filter(|y| buffer[(60, *y)].style().bg == Some(theme.selected))
            .map(|y| {
                (44..99)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .collect::<Vec<_>>()
    };

    for cursor in [2, 4, 1] {
        app.pull_request_step_cursor = cursor;
        let rows = highlighted(&mut app, &mut terminal);
        assert_eq!(
            rows.len(),
            1,
            "exactly one row is highlighted, not a range: {rows:?}"
        );
        assert!(
            rows[0].contains(&format!("Run step {cursor}")),
            "the highlight marks the step the cursor is on: {rows:?}"
        );
    }
}

#[test]
fn a_running_check_shows_the_step_it_is_on_before_any_log_exists() {
    let mut app = overview_app();
    app.pull_request_check_cursor = Some(0);
    app.expanded_check_steps.insert(2);
    app.pull_request_step_cursor = 2;
    app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
        truncated: false,
        unavailable: None,
        log_pending: true,
        loose_lines: Vec::new(),
        steps: vec![
            CheckStep {
                number: 1,
                name: "Set up job".to_owned(),
                status: PullRequestCheckStatus::Passed,
                conclusion: "success".to_owned(),
                started_at: "2026-08-02T10:00:00Z".to_owned(),
                completed_at: "2026-08-02T10:00:02Z".to_owned(),
                lines: Vec::new(),
            },
            CheckStep {
                number: 2,
                name: "Run cargo test".to_owned(),
                status: PullRequestCheckStatus::Pending,
                conclusion: String::new(),
                started_at: "2026-08-02T10:00:02Z".to_owned(),
                completed_at: String::new(),
                lines: Vec::new(),
            },
        ],
    });

    let rendered = check_run_rows(&app, 100, &Theme::default())
        .iter()
        .map(|row| row.line.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("2 steps"));
    assert!(
        rendered.contains("Set up job") && rendered.contains("2s"),
        "a finished step still reports how long it took"
    );
    assert!(
        rendered.contains("Run cargo test"),
        "the step in progress is visible rather than hidden behind the missing log"
    );
    assert!(rendered.contains("waiting for output…"));
    assert!(
        rendered.contains("Waiting for the runner"),
        "the view says why there is no output yet instead of looking broken"
    );
}

#[test]
fn a_comment_shows_its_code_intact_and_only_that_code_scrolls() {
    let mut app = overview_app();
    let wide = "│ ✓ Format, lint, and test (ubuntu-latest)   CI   passed in 33s   https://github.com/acme/widget/actions/runs/1/job/2 │";
    app.pull_request_conversation = crate::git::github::PullRequestConversation {
        truncated: false,
        from_cache: false,
        entries: vec![ConversationEntry {
            kind: ConversationKind::Comment,
            actor: "pulkitxm".to_owned(),
            timestamp: "2026-08-14T20:08:00Z".to_owned(),
            detail: String::new(),
            body: format!(
                "webhook path, driven against this PR with a body long enough to wrap.\n\n```\n{wide}\n  short line\n```"
            ),
            url: String::new(),
            reference: String::new(),
            context: String::new(),
        }],
    };

    let rows = conversation_rows(&app, 80, &Theme::default());
    let text = |row: &ContentRow| row.line.to_string();

    assert!(
        !rows.iter().any(|row| text(row).contains("```")),
        "fence markers are punctuation for a parser, never shown to a reader"
    );
    let scrollable: Vec<String> = rows.iter().filter(|row| row.wide).map(text).collect();
    assert!(scrollable.iter().any(|row| row.contains("cargo test")));
    assert!(scrollable.iter().any(|row| row.contains("  short line")));
    assert!(
        scrollable.iter().any(|row| row.contains("State")
            && row.contains(&format!(
                "opened {}",
                format_local_timestamp("2026-08-01T09:00:00Z")
            ))),
        "a single-line value that outgrows the pane scrolls rather than being clipped"
    );
    let long = rows
        .iter()
        .find(|row| row.wide && text(row).contains(wide))
        .expect("code keeps its full width rather than being cut at the pane");
    assert!(
        rows.iter()
            .filter(|row| !row.wide)
            .all(|row| row.line.width() <= 80),
        "everything that is not code is wrapped to fit"
    );

    let prose = rows
        .iter()
        .find(|row| !row.wide && text(row).contains("webhook path"))
        .expect("the comment body is rendered");
    assert_eq!(
        shift_line(&prose.line, 0, 80).to_string(),
        prose.line.to_string()
    );
    let shifted = shift_line(&long.line, 60, 80).to_string();
    assert!(
        shifted.contains("33s") && shifted.contains("job/2"),
        "scrolling reaches the tail of a line the pane cannot hold: {shifted:?}"
    );
    assert!(!shifted.contains("Format, lint"));
    assert!(
        shift_line(&long.line, 0, 80).to_string().len() < long.line.to_string().len(),
        "the unscrolled view is still clipped to the pane"
    );
}

#[test]
fn the_pane_says_whether_it_is_refreshing_or_showing_a_cached_answer() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    app.pull_request_exact_number = Some(42);
    let theme = Theme::default();
    let render = |app: &mut App, terminal: &mut Terminal<TestBackend>| {
        terminal.draw(|frame| draw(frame, app, &theme)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>()
    };
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();

    app.pull_request_conversation_loading = true;
    let refreshing = render(&mut app, &mut terminal);
    assert!(
        refreshing.contains("loading"),
        "a read in flight says that it is loading"
    );
    assert!(!refreshing.contains(['⟳', '↻', '↺']));
    assert!(!refreshing.contains("cached"));

    app.pull_request_conversation_loading = false;
    app.pull_request_from_cache = true;
    app.pull_request_checks_from_cache = true;
    app.pull_request_conversation.from_cache = true;
    let cached = render(&mut app, &mut terminal);
    assert!(
        cached.contains("cached"),
        "an answer served from disk says so rather than pretending to be live"
    );
    assert!(!cached.contains(['⟳', '↻', '↺']));

    app.pull_request_checks_from_cache = false;
    let live_checks = render(&mut app, &mut terminal);
    assert!(
        live_checks.contains("cached"),
        "a freshly read check list does not make the pull request itself live"
    );

    app.pull_request_conversation.from_cache = false;
    let live = render(&mut app, &mut terminal);
    assert!(!live.contains("cached"));
    assert!(!live.contains(['⟳', '↻', '↺']));
}

#[test]
fn a_failed_lookup_stays_readable_after_its_toast_expires() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_exact_number = Some(404);
    app.pull_request_error =
        Some("unable to load pull request: GraphQL: Could not resolve to a PullRequest".into());
    let mut terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();

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

    assert!(rendered.contains("could not be opened"));
    assert!(rendered.contains("Could not resolve to a PullRequest"));
    assert!(rendered.contains("Press r to try again"));
}

#[test]
fn pull_request_overview_reads_as_a_conversation_beside_its_checks() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    app.pull_request_conversation = crate::git::github::PullRequestConversation {
        truncated: false,
        from_cache: false,
        entries: vec![
            ConversationEntry {
                kind: ConversationKind::Opened,
                actor: "octocat".to_owned(),
                timestamp: "2026-08-01T09:00:00Z".to_owned(),
                detail: "feature/rocket into main".to_owned(),
                body: "## Summary".to_owned(),
                url: String::new(),
                reference: String::new(),
                context: String::new(),
            },
            ConversationEntry {
                kind: ConversationKind::ForcePush,
                actor: "octocat".to_owned(),
                timestamp: "2026-08-02T09:10:00Z".to_owned(),
                detail: String::new(),
                body: String::new(),
                url: String::new(),
                reference: "deadbeefcafe".to_owned(),
                context: String::new(),
            },
            ConversationEntry {
                kind: ConversationKind::ReviewComment,
                actor: "reviewer".to_owned(),
                timestamp: "2026-08-02T09:30:00Z".to_owned(),
                detail: "src/main.rs:42".to_owned(),
                body: "Extract this into a helper".to_owned(),
                url: String::new(),
                reference: String::new(),
                context: "@@ -1 +1 @@\n-old\n+new".to_owned(),
            },
        ],
    };
    let mut terminal = Terminal::new(TestBackend::new(150, 40)).unwrap();

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

    assert!(rendered.contains("Conversation"));
    assert!(rendered.contains("Format, lint, and test"));
    assert!(rendered.contains("#42"));
    assert!(rendered.contains("acme/widget:main"));
    assert!(rendered.contains(&format_local_timestamp("2026-08-01T09:00:00Z")));
    assert!(rendered.contains("Description"));
    assert!(
        rendered.contains("Launch safely"),
        "the body renders as prose, not as raw Markdown"
    );
    assert!(rendered.contains("cargo test"), "fenced code survives");
    assert!(rendered.contains("opened this pull request"));
    assert!(rendered.contains("force-pushed to deadbee"));
    assert!(rendered.contains("commented on src/main.rs:42"));
    assert!(rendered.contains("Extract this into a helper"));
    assert!(
        !rendered.contains("## Summary"),
        "the opening post never repeats the description above it"
    );
}

#[test]
fn selecting_a_check_shows_its_steps_and_opens_the_failure() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = overview_app();
    app.pull_request_check_cursor = Some(0);
    app.pull_request_step_cursor = 2;
    app.expanded_check_steps.insert(2);
    app.pull_request_check_log = Some(crate::git::github::CheckRunLog {
        truncated: false,
        unavailable: None,
        log_pending: false,
        loose_lines: vec![CheckLogLine {
            timestamp: "2026-08-02T10:02:31Z".to_owned(),
            text: "Cleaning up runner".to_owned(),
            severity: CheckLogSeverity::Normal,
        }],
        steps: vec![
            CheckStep {
                number: 1,
                name: "Set up job".to_owned(),
                status: PullRequestCheckStatus::Passed,
                conclusion: "success".to_owned(),
                started_at: "2026-08-02T10:00:00Z".to_owned(),
                completed_at: "2026-08-02T10:00:02Z".to_owned(),
                lines: vec![CheckLogLine {
                    timestamp: "2026-08-02T10:00:01Z".to_owned(),
                    text: "hidden while folded".to_owned(),
                    severity: CheckLogSeverity::Normal,
                }],
            },
            CheckStep {
                number: 2,
                name: "Run cargo test".to_owned(),
                status: PullRequestCheckStatus::Failed,
                conclusion: "failure".to_owned(),
                started_at: "2026-08-02T10:00:02Z".to_owned(),
                completed_at: "2026-08-02T10:02:30Z".to_owned(),
                lines: vec![CheckLogLine {
                    timestamp: "2026-08-02T10:02:29Z".to_owned(),
                    text: "test tests::rockets ... FAILED".to_owned(),
                    severity: CheckLogSeverity::Error,
                }],
            },
        ],
    });
    let mut terminal = Terminal::new(TestBackend::new(150, 40)).unwrap();

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

    assert!(rendered.contains("2 steps"));
    assert!(rendered.contains("Set up job"));
    assert!(rendered.contains("Run cargo test"));
    assert!(
        rendered.contains("2m 28s"),
        "a step reports how long it ran"
    );
    assert!(rendered.contains("test tests::rockets ... FAILED"));
    assert!(
        !rendered.contains("hidden while folded"),
        "a folded step keeps its output out of the way"
    );
    assert!(rendered.contains("Runner output"));
    assert!(rendered.contains("Cleaning up runner"));
    assert!(
        !app.geometry.content_step_hits.is_empty(),
        "step rows stay clickable"
    );

    let rows = check_run_rows(&app, 40, &Theme::default());
    let log = rows
        .iter()
        .find(|row| row.line.to_string().contains("rockets"))
        .expect("the expanded step's output is rendered");
    assert!(log.wide, "a log line scrolls instead of being truncated");
    assert!(
        log.line
            .to_string()
            .ends_with("test tests::rockets ... FAILED"),
        "the line keeps its full text even in a pane far narrower than it"
    );
}

#[test]
fn pull_request_loading_renders_on_demand_progress_and_skeletons() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.pull_request_loading = true;
    app.pull_request_exact_number = Some(42);
    app.pull_request_lookup = crate::app::TextBuffer::new("42");
    app.pull_request_progress = Some(crate::git::github::PullRequestProgress::FetchingHead);
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let rendered: String = buffer
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    let bottom = (24..27)
        .flat_map(|row| (0..42).map(move |column| buffer[(column, row)].symbol()))
        .collect::<String>();

    assert!(rendered.contains("50%"));
    assert!(rendered.contains("Fetching the source commit"));
    assert!(rendered.contains('█'));
    assert!(bottom.contains("auto-detect"));
    assert!(bottom.contains("PR #"));
}

#[test]
fn empty_pull_request_view_renders_recent_numbers_and_titles_as_rows() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.view = View::PullRequests;
    app.recent_pull_requests = vec![RecentPullRequest {
        number: 39,
        title: "Restore selectable previews".to_owned(),
        repository: GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://github.com/acme/widget".to_owned(),
            remotes: vec!["origin".to_owned()],
        },
    }];
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();

    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();

    assert!(rendered.contains("Recent Pull Requests"));
    assert!(rendered.contains("#39 Restore selectable previews"));
    assert!(
        app.geometry
            .sidebar_hits
            .iter()
            .any(|hit| { matches!(hit.target, SidebarHit::RecentPullRequest(0)) })
    );
}

#[test]
fn file_header_right_aligns_colored_line_counts() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let header = test_file_header("src/main.rs", 12, 3);
    let theme = Theme::default();
    let backend = TestBackend::new(40, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new("/tmp/repo", "repo");
    app.document.lines = vec![header.clone()];
    terminal
        .draw(|frame| draw_file_header(frame, frame.area(), &header, &app, &theme))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let rendered: String = buffer
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    let addition_column = rendered
        .chars()
        .position(|character| character == '+')
        .unwrap();
    let deletion_column = rendered
        .chars()
        .position(|character| character == '-')
        .unwrap();

    assert!(rendered.ends_with("+12 -3 ─"), "{rendered:?}");
    assert!(rendered.contains('─'));
    assert!(rendered.contains("\u{e7a8} src/main.rs"));
    assert!(!rendered.contains(['┌', '┐', '└', '┘', '│']));
    assert!(!rendered.contains('⌄'));
    assert!(!rendered.contains('›'));
    assert_eq!(buffer[(cells(addition_column), 0)].fg, theme.added);
    assert_eq!(buffer[(cells(deletion_column), 0)].fg, theme.removed);
}

#[test]
fn file_footer_draws_one_horizontal_separator_without_vertical_edges() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let theme = Theme::default();
    let mut terminal = Terminal::new(TestBackend::new(12, 1)).unwrap();
    terminal
        .draw(|frame| draw_file_footer(frame, frame.area(), &theme))
        .unwrap();

    let buffer = terminal.backend().buffer();
    assert!(
        buffer
            .content()
            .iter()
            .all(|cell| cell.symbol() == "─" && cell.fg == theme.border)
    );
}

#[test]
fn skips_intraline_work_for_very_long_rows() {
    let old = test_line(
        DiffLineKind::Removed,
        &"a".repeat(MAX_INTRALINE_SOURCE_BYTES + 1),
    );
    let new = test_line(
        DiffLineKind::Added,
        &"b".repeat(MAX_INTRALINE_SOURCE_BYTES + 1),
    );

    assert_eq!(
        paired_intraline_emphasis(Some(&old), Some(&new)),
        (None, None)
    );
}

#[test]
fn visible_intraline_emphasis_matches_block_pairing() {
    let lines = vec![
        test_line(DiffLineKind::Context, "same"),
        test_line(DiffLineKind::Removed, "value = 1"),
        test_line(DiffLineKind::Removed, "gone"),
        test_line(DiffLineKind::Added, "value = 2"),
        test_line(DiffLineKind::Context, "same"),
        test_line(DiffLineKind::Added, "standalone"),
    ];

    let emphasis = visible_intraline_emphasis(&lines, 0..lines.len());
    assert_eq!(emphasis.get(&1), Some(&(8..9)), "removed side of the pair");
    assert_eq!(emphasis.get(&3), Some(&(8..9)), "added side of the pair");
    assert!(!emphasis.contains_key(&2), "unpaired removed line");
    assert!(
        !emphasis.contains_key(&5),
        "added run without a removed partner"
    );

    let only_added = visible_intraline_emphasis(&lines, [3_usize].into_iter());
    assert_eq!(
        only_added.get(&3),
        Some(&(8..9)),
        "partner is found outside the viewport"
    );
}

#[test]
fn document_details_participate_in_vertical_scroll() {
    assert_eq!(commit_details_row_count(30), 7);
    assert_eq!(commit_details_row_count(8), 5);
    assert_eq!(commit_details_row_count(3), 0);
    assert_eq!(pull_request_details_row_count(30), 12);
    assert_eq!(pull_request_details_row_count(9), 6);
    assert_eq!(pull_request_details_row_count(3), 0);
}

#[test]
fn pull_request_description_preview_is_bounded_and_marks_truncation() {
    let lines =
        description_preview_lines("one two three four five six seven eight nine ten", 10, 3);
    assert_eq!(lines.len(), 3);
    assert!(lines.last().unwrap().ends_with('…'));
    assert!(lines.iter().all(|line| line.width() <= 10));
}

#[test]
fn pull_request_description_preview_cleans_common_markdown() {
    let lines = description_preview_lines(
        "## Summary\n- Add a **Pull Requests** tab\n- Cache raw `gh` metadata",
        24,
        3,
    );
    let rendered = lines.join(" ");

    assert!(rendered.contains("Add a Pull Requests tab"));
    assert!(rendered.contains("•"));
    assert!(!rendered.contains("Summary"));
    assert!(!rendered.contains('#'));
    assert!(!rendered.contains('*'));
    assert!(!rendered.contains('`'));
    assert!(lines.iter().all(|line| line.width() <= 24));
}

#[test]
fn formats_zero_one_and_multiple_remote_aliases() {
    assert_eq!(remote_suffix(&[]), "");
    assert_eq!(remote_suffix(&["origin".to_owned()]), "  ·  remote origin");
    assert_eq!(
        remote_suffix(&["origin".to_owned(), "upstream".to_owned()]),
        "  ·  remotes origin, upstream"
    );
}

#[test]
fn collapse_all_keeps_only_selectable_file_headers() {
    let document = DiffDocument {
        title: String::new(),
        truncated: false,
        commit_details: None,
        pull_request_details: None,
        lines: vec![
            test_file_header("one.rs", 1, 0),
            test_line(DiffLineKind::HunkHeader, "@@ -0,0 +1 @@"),
            test_line(DiffLineKind::Added, "one"),
            test_line(DiffLineKind::FileFooter, ""),
            test_file_header("two.rs", 1, 0),
            test_line(DiffLineKind::Added, "two"),
            test_line(DiffLineKind::FileFooter, ""),
        ],
    };

    let mut app = App::new("/tmp/repo", "repo");
    app.document = document.clone();
    app.files_collapsed = true;
    assert_eq!(unified_row_indices(&document, &app), vec![0, 4]);
    assert_eq!(side_by_side_rows(&document, &app).len(), 2);
}

#[test]
fn computes_vscode_style_intraline_changed_ranges() {
    assert_eq!(
        changed_ranges("const oldValue = 1;", "const newValue = 2;"),
        (Some(6..18), Some(6..18))
    );
    assert_eq!(changed_ranges("same", "same"), (None, None));
    assert_eq!(
        changed_ranges("prefix old suffix", "prefix new suffix"),
        (Some(7..10), Some(7..10))
    );
}

fn test_file_header(path: &str, additions: usize, deletions: usize) -> DiffLine {
    DiffLine {
        kind: DiffLineKind::FileHeader,
        old_line: None,
        new_line: None,
        spans: vec![
            HighlightSpan {
                text: path.to_owned(),
                foreground: None,
                bold: false,
                italic: false,
            },
            HighlightSpan {
                text: format!("+{additions}"),
                foreground: None,
                bold: false,
                italic: false,
            },
            HighlightSpan {
                text: format!("-{deletions}"),
                foreground: None,
                bold: false,
                italic: false,
            },
        ],
    }
}

fn test_line(kind: DiffLineKind, text: &str) -> DiffLine {
    DiffLine {
        kind,
        old_line: None,
        new_line: None,
        spans: vec![HighlightSpan {
            text: text.to_owned(),
            foreground: None,
            bold: false,
            italic: false,
        }],
    }
}
