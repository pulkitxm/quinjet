use std::path::PathBuf;
use std::time::Instant;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;
use crate::app::AppEffect;
use crate::git::github::PullRequestDiffIndex;
use crate::git::worker::{WorkerCommand, WorkerEvent};

fn indexed_app(files: usize, layout: DiffLayout) -> App {
    let mut app = overview_app();
    app.pull_request_section = PullRequestSection::Files;
    app.diff_layout = layout;
    app.diff_generation = 10;
    app.files_collapsed = false;
    app.collapse_preference_set = true;
    app.handle_worker_event(
        WorkerEvent::PullRequestIndex {
            generation: 10,
            result: Ok(PullRequestDiffIndex {
                files: (0..files)
                    .map(|index| PullRequestFile {
                        path: PathBuf::from(format!("file-{index:04}.txt")),
                        old_path: None,
                        status: PullRequestFileStatus::Added,
                        counts: None,
                    })
                    .collect(),
                total_files: files,
                truncated: false,
            }),
        },
        Instant::now(),
    );
    app
}

fn deliver_file(app: &mut App, path: &str, body_rows: usize) {
    let mut lines = vec![test_file_header(path, body_rows, 0)];
    lines.extend(
        (0..body_rows)
            .map(|row| test_line(DiffLineKind::Added, &format!("synthetic content {row}"))),
    );
    lines.push(test_line(DiffLineKind::FileFooter, ""));
    app.handle_worker_event(
        WorkerEvent::PullRequestDiffBatch {
            workspace_generation: 10,
            result: Ok(vec![(
                PathBuf::from(path),
                DiffDocument {
                    lines,
                    ..DiffDocument::default()
                },
            )]),
        },
        Instant::now(),
    );
}

#[test]
fn large_pr_rendering_loads_only_the_content_viewport_in_both_layouts() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = indexed_app(4_262, layout);
        let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        assert_eq!(app.document.file_count(), 4_262);
        assert!(app.pull_request_viewport_paths.len() < 40);
        assert_eq!(
            app.pull_request_viewport_paths[0],
            PathBuf::from("file-0000.txt")
        );

        app.content_scroll = usize::MAX;
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let (effects, _) = app.tick(Instant::now());
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestFileBatch { paths, .. }
                    if paths.iter().all(|path| path > &PathBuf::from("file-4200.txt"))
                        && paths.iter().all(|path| app.pull_request_viewport_paths.contains(path))
            )
        ));
        assert!(
            app.pull_request_viewport_paths
                .contains(&PathBuf::from("file-4261.txt"))
        );
        assert_eq!(app.sidebar_offset, 0);
    }
}

#[test]
fn a_viewport_inside_a_large_file_tracks_its_sticky_header() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = indexed_app(4_262, layout);
        app.wrap_code = true;
        deliver_file(&mut app, "file-0000.txt", 1_000);
        app.content_scroll = 900;
        let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        assert_eq!(
            app.pull_request_viewport_paths,
            [PathBuf::from("file-0000.txt")]
        );
        assert!(app.tick(Instant::now()).0.is_empty());
    }
}

#[test]
fn earlier_patches_preserve_the_visible_file_and_row_offset() {
    for layout in [DiffLayout::Unified, DiffLayout::SideBySide] {
        let mut app = indexed_app(100, layout);
        app.content_scroll = 19;
        let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let (path, offset, scroll) = app.pull_request_scroll_anchor.clone().unwrap();
        deliver_file(&mut app, "file-0000.txt", 100);
        terminal
            .draw(|frame| draw(frame, &mut app, &Theme::default()))
            .unwrap();
        let restored = app.pull_request_scroll_anchor.clone().unwrap();
        assert_eq!(restored.0, path);
        assert_eq!(restored.1, offset);
        assert!(app.content_scroll > scroll);
    }
}

#[test]
fn navigation_after_a_patch_arrives_takes_priority_over_scroll_restoration() {
    let mut app = indexed_app(100, DiffLayout::Unified);
    app.content_scroll = 19;
    let mut terminal = Terminal::new(TestBackend::new(180, 48)).unwrap();
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    deliver_file(&mut app, "file-0000.txt", 100);
    app.content_scroll = 0;
    terminal
        .draw(|frame| draw(frame, &mut app, &Theme::default()))
        .unwrap();
    assert_eq!(app.content_scroll, 0);
}
