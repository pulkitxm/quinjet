use std::io::{self, Write};
use std::time::Instant;

use anyhow::{Context, Result};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use super::*;

fn fixture(files: usize, body_rows: usize) -> App {
    let mut app = App::new("/example/project", "example");
    let mut lines = Vec::new();
    for index in 0..files {
        let path = format!("example_{index:04}.rs");
        lines.push(test_file_header(&path, 1, 0));
        lines.extend((0..body_rows).map(|_| test_line(DiffLineKind::Context, "let value = 42;")));
        lines.push(test_line(DiffLineKind::FileFooter, ""));
        app.status.changes.push(Change {
            path: path.into(),
            original_path: None,
            area: ChangeArea::Unstaged,
            status: ChangeStatus::Modified,
        });
    }
    app.set_document(DiffDocument {
        title: "synthetic benchmark".to_owned(),
        lines,
        ..DiffDocument::default()
    });
    app.document_loading = false;
    app.refreshing = false;
    app.files_collapsed = files > 1;
    app
}

fn measure(label: &str, files: usize, body_rows: usize, deep: bool) -> Result<()> {
    let mut app = fixture(files, body_rows);
    if deep {
        app.content_scroll = body_rows.saturating_sub(100);
    }
    let theme = app.theme;
    let mut terminal = Terminal::new(TestBackend::new(160, 45))?;
    let started = Instant::now();
    let _ = terminal.draw(|frame| draw(frame, &mut app, &theme))?;
    let first_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let mut timings = Vec::new();
    for _ in 0..31 {
        let started = Instant::now();
        let _ = terminal.draw(|frame| draw(frame, &mut app, &theme))?;
        timings.push(started.elapsed().as_secs_f64() * 1_000.0);
        let _ = std::hint::black_box(terminal.backend().buffer());
    }
    timings.sort_by(f64::total_cmp);
    let median = timings.get(15).context("missing median render sample")?;
    let p95 = timings.get(29).context("missing p95 render sample")?;
    writeln!(
        io::stdout().lock(),
        "render {label}: first_ms={first_ms:.3} median_ms={median:.3} p95_ms={p95:.3} samples=31"
    )?;
    Ok(())
}

#[test]
#[ignore = "release-mode render benchmark with synthetic data"]
fn release_render_timings() -> Result<()> {
    for files in [100, 1_000, 4_000] {
        measure(&format!("collapsed_{files}_files"), files, 12, false)?;
    }
    measure("deep_100000_rows", 1, 100_000, true)
}
