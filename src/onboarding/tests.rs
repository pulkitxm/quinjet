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
    assert!(rendered.contains("Ctrl+E all"));
    assert!(!rendered.contains("fatal:"));
}

#[test]
fn project_loading_is_visible_until_background_results_arrive() {
    let mut onboarding = Onboarding::from_groups(Path::new("/tmp"), Vec::new(), None);
    let (sender, receiver) = crossbeam_channel::bounded(1);
    onboarding.project_loader = ProjectLoader::waiting(receiver);
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
    assert!(rendered.contains("Loading projects…"));
    assert!(rendered.contains('◌'));

    sender.send(vec![project_group()]).unwrap();
    assert!(onboarding.poll_projects());
    assert_eq!(
        terminal
            .draw(|frame| onboarding.draw(frame, &theme))
            .unwrap()
            .area,
        Rect::new(0, 0, 100, 30),
    );
    let rendered = terminal.backend().to_string();
    assert!(!rendered.contains("Loading projects…"));
    assert!(rendered.contains("quinjet"));
}

#[test]
fn project_picker_compacts_long_paths_and_toggles_all_projects() {
    let mut group = project_group();
    let full_path = "/Users/pulkit/scripts/quinjet/features/a-very-long-worktree-name";
    group.worktrees[0].path = PathBuf::from(full_path);
    group.worktrees[0].updated_at = Some("2020-08-22T18:00:00Z".to_owned());
    group.worktrees[0].updated_unix = Some(1_598_119_200);
    let mut onboarding = Onboarding::from_groups(Path::new("/tmp"), vec![group], None);
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
    assert!(rendered.contains("Ctrl+E all"));

    drop(onboarding.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL)));
    assert_eq!(
        terminal
            .draw(|frame| onboarding.draw(frame, &theme))
            .unwrap()
            .area,
        Rect::new(0, 0, 100, 30),
    );
    let rendered = terminal.backend().to_string();
    assert!(rendered.contains("[⌄]"));
    assert!(rendered.contains("main"));
    assert!(rendered.contains("Ctrl+E all"));
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
fn local_project_picker_uses_the_inline_machine_strip() {
    let context = SshContext {
        current: "Pulkits-MacBook-Pro.local".to_owned(),
        machines: vec![
            SshMachine {
                target: "Pulkits-MacBook-Pro.local".to_owned(),
                folder: PathBuf::from("/work"),
                accessible: true,
                uses: 0,
                local: true,
            },
            SshMachine {
                target: "busy-host".to_owned(),
                folder: PathBuf::from("/work/busy"),
                accessible: true,
                uses: 8,
                local: false,
            },
            SshMachine {
                target: "offline-host".to_owned(),
                folder: PathBuf::from("/work/offline"),
                accessible: false,
                uses: 2,
                local: false,
            },
        ],
        tabs: crate::ssh::SshTabs::default(),
        probing: false,
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
    assert!(rendered.contains("Machine"));
    assert!(rendered.contains("Pulkits-MacBook-Pro.local"));
    assert!(rendered.contains("busy-host"));
    assert!(rendered.contains("Tab machine"));
    let button = onboarding
        .machine_hits
        .iter()
        .find(|(_, index)| *index == 1)
        .map(|(area, _)| *area)
        .expect("SSH machine chip");

    assert_eq!(
        onboarding.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: button.x,
            row: button.y,
            modifiers: KeyModifiers::NONE,
        }),
        OnboardingAction::SwitchSshMachine(SshSwitch {
            index: 1,
            mode: SshProjectOpenMode::Current,
        })
    );
    assert_eq!(
        onboarding.handle_key(key(KeyCode::Tab)),
        OnboardingAction::SwitchSshMachine(SshSwitch {
            index: 1,
            mode: SshProjectOpenMode::Current,
        })
    );
}

#[test]
fn new_tab_onboarding_keeps_its_pending_identity_across_switch_and_cancel() {
    let machines = vec![
        SshMachine {
            target: "macbook".to_owned(),
            folder: PathBuf::from("/local"),
            accessible: true,
            uses: 0,
            local: true,
        },
        SshMachine {
            target: "tof".to_owned(),
            folder: PathBuf::from("/remote"),
            accessible: true,
            uses: 2,
            local: false,
        },
    ];
    let mut tabs = crate::ssh::SshTabs::default();
    let local = tabs.append("macbook", "local", "/local/repo");
    let pending = tabs.append_pending("macbook", "/local/repo");
    let context = SshContext {
        current: "macbook".to_owned(),
        machines,
        tabs,
        probing: false,
    };
    let mut switching = Onboarding::from_groups_with_mode(
        Path::new("/local"),
        Vec::new(),
        Some(context.clone()),
        ProjectOpenMode::NewTab,
    );

    assert_eq!(
        switching.handle_key(key(KeyCode::Tab)),
        OnboardingAction::SwitchSshMachine(SshSwitch {
            index: 1,
            mode: SshProjectOpenMode::New,
        })
    );
    assert_eq!(
        switching
            .ssh_context()
            .and_then(|saved| saved.tabs.get(pending).map(|tab| tab.machine.clone())),
        Some("tof".to_owned())
    );

    let mut canceling = Onboarding::from_groups_with_mode(
        Path::new("/local"),
        Vec::new(),
        Some(context),
        ProjectOpenMode::NewTab,
    );
    assert_eq!(
        canceling.handle_key(key(KeyCode::Esc)),
        OnboardingAction::SwitchSshMachine(SshSwitch {
            index: 0,
            mode: SshProjectOpenMode::Activate,
        })
    );
    let saved = canceling.ssh_context().expect("updated context");
    assert!(saved.tabs.get(pending).is_none());
    assert_eq!(saved.tabs.active_id(), Some(local));
}
