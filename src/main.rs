mod app;
mod convert;
mod git;
mod ui;
mod watch;
mod webhook;

use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use crossbeam_channel::{Receiver, tick};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::{App, AppEffect};
use crate::git::Repository;
use crate::git::worker::GitWorker;
use crate::watch::RepoWatcher;
use crate::webhook::WebhookListener;

#[derive(Debug, Parser)]
#[command(name = "quinjet", version, about)]
struct Cli {
    /// Git repository to open
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Disable mouse capture (all features remain keyboard-accessible)
    #[arg(long)]
    no_mouse: bool,

    /// Refresh the open pull request the moment a forwarded GitHub webhook
    /// arrives, given a port or host:port to listen on. Pair with
    /// `gh webhook forward --repo <repo> --events '*' --url http://127.0.0.1:<port>`.
    /// Only loopback connections are accepted.
    #[arg(long, value_name = "ADDRESS")]
    webhook_listen: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        anyhow::bail!("Quinjet requires an interactive terminal");
    }

    let repository = Repository::discover(&cli.path)?;
    let worker = GitWorker::start(repository.clone());
    let watcher = RepoWatcher::new(repository.root()).ok();
    let webhooks = cli
        .webhook_listen
        .as_deref()
        .map(WebhookListener::bind)
        .transpose()?;
    let mut app = App::new(repository.root(), repository.name());
    let mut terminal = TerminalGuard::enter(!cli.no_mouse)?;
    let render_tick = tick(Duration::from_millis(16));
    let periodic_refresh = tick(Duration::from_secs(10));
    let mut dirty = true;
    let mut running = true;

    app.mouse_capture = !cli.no_mouse;
    app.webhooks_listening = webhooks.is_some();
    running &= dispatch_effects(&worker, &mut terminal, app.initial_effects());
    while running {
        if dirty {
            let _ = terminal
                .terminal
                .draw(|frame| ui::draw(frame, &mut app))
                .context("failed to render Quinjet")?;
            dirty = false;
        }

        while let Ok(worker_event) = worker.events().try_recv() {
            let effects = app.handle_worker_event(worker_event, Instant::now());
            running &= dispatch_effects(&worker, &mut terminal, effects);
            dirty = true;
        }

        if watcher_changed(watcher.as_ref().map(RepoWatcher::changes)) {
            let mut effects = Vec::new();
            app.filesystem_changed(&mut effects);
            running &= dispatch_effects(&worker, &mut terminal, effects);
            dirty = true;
        }

        if webhook_delivered(webhooks.as_ref()) {
            running &= dispatch_effects(
                &worker,
                &mut terminal,
                app.webhook_delivered(Instant::now()),
            );
            dirty = true;
        }

        if render_tick.try_recv().is_ok() {
            let (effects, changed) = app.tick(Instant::now());
            running &= dispatch_effects(&worker, &mut terminal, effects);
            dirty |= changed;
        }
        if periodic_refresh.try_recv().is_ok() {
            let mut effects = Vec::new();
            app.periodic_refresh(&mut effects);
            running &= dispatch_effects(&worker, &mut terminal, effects);
            dirty = true;
        }

        if event::poll(Duration::from_millis(8)).context("failed to poll terminal events")? {
            let effects = match event::read().context("failed to read terminal event")? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    app.handle_key(key, Instant::now())
                }
                Event::Mouse(mouse) => app.handle_mouse(mouse, Instant::now()),
                Event::Paste(text) => {
                    app.handle_paste(&text);
                    Vec::new()
                }
                _ => Vec::new(),
            };
            running &= dispatch_effects(&worker, &mut terminal, effects);
            dirty = true;
        }
    }

    Ok(())
}

/// Handing a URL to the desktop is best effort: the toast has already said
/// which one, so a machine with no opener leaves the reader able to copy it
/// rather than facing an error they cannot act on.
#[expect(
    clippy::disallowed_methods,
    reason = "handing a URL to the desktop opener is not a Git subprocess"
)]
fn open_url(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    drop(
        Command::new(opener)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn(),
    );
}

fn watcher_changed(receiver: Option<&Receiver<()>>) -> bool {
    let Some(receiver) = receiver else {
        return false;
    };
    let mut changed = false;
    while receiver.try_recv().is_ok() {
        changed = true;
    }
    changed
}

/// Deliveries only say that something changed, so several arriving together
/// collapse into the single refresh they would each have asked for.
fn webhook_delivered(listener: Option<&WebhookListener>) -> bool {
    let Some(listener) = listener else {
        return false;
    };
    let mut delivered = false;
    while listener.deliveries().try_recv().is_ok() {
        delivered = true;
    }
    delivered
}

fn dispatch_effects(
    worker: &GitWorker,
    terminal: &mut TerminalGuard,
    effects: Vec<AppEffect>,
) -> bool {
    let mut running = true;
    for effect in effects {
        match effect {
            AppEffect::Git(command) => {
                running &= worker.send(*command);
            }
            AppEffect::SetMouseCapture(enabled) => terminal.set_mouse_capture(enabled),
            AppEffect::OpenUrl(url) => open_url(&url),
            AppEffect::Quit => running = false,
        }
    }
    running
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    mouse: bool,
    keyboard_enhancements: bool,
}

impl TerminalGuard {
    /// A terminal cannot select text with a mouse it is reporting to the
    /// application, so releasing it is the only way to make the screen
    /// selectable and copyable.
    fn set_mouse_capture(&mut self, enabled: bool) {
        if self.mouse == enabled {
            return;
        }
        let backend = self.terminal.backend_mut();
        let applied = if enabled {
            execute!(backend, EnableMouseCapture)
        } else {
            execute!(backend, DisableMouseCapture)
        };
        if applied.is_ok() {
            self.mouse = enabled;
        }
    }

    fn enter(mouse: bool) -> Result<Self> {
        enable_raw_mode().context("failed to enable terminal raw mode")?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)
            .context("failed to enter alternate screen")?;
        if mouse {
            execute!(stdout, EnableMouseCapture).context("failed to enable mouse capture")?;
        }
        let keyboard_enhancements =
            crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false);
        if keyboard_enhancements {
            drop(execute!(
                stdout,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                )
            ));
        }
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("failed to initialize terminal")?;
        terminal.clear().context("failed to clear terminal")?;
        Ok(Self {
            terminal,
            mouse,
            keyboard_enhancements,
        })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        drop(disable_raw_mode());
        if self.keyboard_enhancements {
            drop(execute!(
                self.terminal.backend_mut(),
                PopKeyboardEnhancementFlags
            ));
        }
        if self.mouse {
            drop(execute!(self.terminal.backend_mut(), DisableMouseCapture));
        }
        drop(execute!(
            self.terminal.backend_mut(),
            DisableBracketedPaste,
            LeaveAlternateScreen
        ));
        drop(self.terminal.show_cursor());
    }
}
