use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;
use crate::git::Worktree;
use crate::ssh::SshMachine;
use crate::theme::{Appearance, ThemeName};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn project_group() -> ProjectGroup {
    ProjectGroup {
        name: "quinjet".to_owned(),
        common_dir: PathBuf::from("/repos/quinjet/.git"),
        worktrees: vec![Worktree {
            path: PathBuf::from("/repos/quinjet"),
            head: "0123456789abcdef".to_owned(),
            updated_at: Some("2026-08-22T18:00:00Z".to_owned()),
            updated_unix: Some(1_776_964_800),
            branch: Some("main".to_owned()),
            current: false,
            bare: false,
            detached: false,
            locked: None,
            prunable: None,
        }],
    }
}

#[test]
fn empty_screen_reuses_the_grouped_project_picker() {
    let mut onboarding = Onboarding::from_groups(Path::new("/tmp"), vec![project_group()], None);
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::new(ThemeName::Quinjet, Appearance::Dark);
    assert_eq!(
        terminal
            .draw(|frame| onboarding.draw(frame, &theme))
            .unwrap()
            .area,
        Rect::new(0, 0, 100, 30),
    );
    let rendered = terminal.backend().to_string();
    assert!(rendered.contains("Open a project"));
    assert!(rendered.contains("quinjet"));
    assert!(rendered.contains("main"));
    assert!(rendered.contains("/repos/quinjet"));
    assert!(rendered.contains("[⌄]"));
    assert!(rendered.contains("Ctrl+E collapse all"));
    assert!(!rendered.contains("fatal:"));
}

#[test]
fn project_picker_compacts_long_paths_and_collapses_all() {
    let mut group = project_group();
    let full_path = "/Users/pulkit/scripts/quinjet/features/a-very-long-worktree-name";
    group.worktrees[0].path = PathBuf::from(full_path);
    group.worktrees[0].updated_at = Some("2020-08-22T18:00:00Z".to_owned());
    group.worktrees[0].updated_unix = Some(1_598_119_200);
    let mut onboarding = Onboarding::from_groups(Path::new("/tmp"), vec![group]);
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    let theme = Theme::new(ThemeName::Quinjet, Appearance::Dark);

    assert_eq!(
        terminal
            .draw(|frame| onboarding.draw(frame, &theme))
            .unwrap()
            .area,
        Rect::new(0, 0, 100, 30),
    );
    let rendered = terminal.backend().to_string();
    assert!(rendered.contains('…'));
    assert!(!rendered.contains(full_path));
    assert!(rendered.contains("years ago"));

    drop(onboarding.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL)));
    assert_eq!(
        terminal
            .draw(|frame| onboarding.draw(frame, &theme))
            .unwrap()
            .area,
        Rect::new(0, 0, 100, 30),
    );
    let rendered = terminal.backend().to_string();
    assert!(rendered.contains("[›]"));
    assert!(!rendered.contains("main"));
}

#[test]
fn project_picker_opens_the_selected_worktree() {
    let mut onboarding = Onboarding::from_groups(Path::new("/tmp"), vec![project_group()], None);
    assert_eq!(
        onboarding.handle_key(key(KeyCode::Enter)),
        OnboardingAction::Open(PathBuf::from("/repos/quinjet"))
    );
}

#[test]
fn path_entry_resolves_relative_to_the_launch_directory() {
    let mut onboarding = Onboarding::from_groups(Path::new("/work"), Vec::new(), None);
    drop(onboarding.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)));
    for character in "project".chars() {
        drop(onboarding.handle_key(key(KeyCode::Char(character))));
    }
    assert_eq!(
        onboarding.handle_key(key(KeyCode::Enter)),
        OnboardingAction::Open(PathBuf::from("/work/project"))
    );
}

#[test]
fn pasted_control_characters_are_removed_from_paths() {
    let mut onboarding = Onboarding::from_groups(Path::new("/work"), Vec::new(), None);
    drop(onboarding.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)));
    onboarding.handle_paste("repo\nname\u{0000}");
    assert_eq!(onboarding.path_input, "reponame");
}

#[test]
fn local_project_picker_opens_the_ranked_machine_switcher() {
    let context = SshContext {
        current: "local".to_owned(),
        machines: vec![
            SshMachine {
                target: "busy-host".to_owned(),
                folder: PathBuf::from("/work/busy"),
                accessible: true,
                uses: 8,
            },
            SshMachine {
                target: "offline-host".to_owned(),
                folder: PathBuf::from("/work/offline"),
                accessible: false,
                uses: 2,
            },
        ],
    };
    let mut onboarding = Onboarding::from_groups(Path::new("/work"), Vec::new(), Some(context));
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::new(ThemeName::Quinjet, Appearance::Dark);

    assert_eq!(
        terminal
            .draw(|frame| onboarding.draw(frame, &theme))
            .unwrap()
            .area,
        Rect::new(0, 0, 100, 30),
    );
    let rendered = terminal.backend().to_string();
    assert!(rendered.contains("Machine: local"));
    let button = onboarding.machine_button.expect("machine button");

    assert_eq!(
        onboarding.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: button.x,
            row: button.y,
            modifiers: KeyModifiers::NONE,
        }),
        OnboardingAction::None
    );
    assert_eq!(
        terminal
            .draw(|frame| onboarding.draw(frame, &theme))
            .unwrap()
            .area,
        Rect::new(0, 0, 100, 30),
    );
    let rendered = terminal.backend().to_string();
    assert!(rendered.contains("Switch machine"));
    assert!(rendered.contains("busy-host"));
    assert!(rendered.contains("used 8"));
    assert_eq!(
        onboarding.handle_key(key(KeyCode::Enter)),
        OnboardingAction::SwitchSshMachine(0)
    );
}
