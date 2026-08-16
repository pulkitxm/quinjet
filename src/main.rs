mod app;
mod cli;
mod convert;
mod git;
mod ui;
mod watch;
mod webhook;

use std::io::{self, IsTerminal};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, tick};
use crossterm::cursor::Show;
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
use crate::cli::{Launch, TerminalOptions};
use crate::git::Repository;
use crate::git::worker::GitWorker;
use crate::watch::RepoWatcher;
use crate::webhook::WebhookListener;

fn main() -> ExitCode {
    match cli::dispatch() {
        Ok(Launch::Terminal(options)) => match open_terminal(&options) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => ExitCode::from(cli::report(&error)),
        },
        Ok(Launch::Finished(code)) => ExitCode::from(code),
        Err(error) => ExitCode::from(cli::report(&error)),
    }
}

/// Whether the alternate screen is currently ours to leave.
static TERMINAL_ENTERED: AtomicBool = AtomicBool::new(false);
/// Whether a keyboard enhancement entry is ours to pop.
static KEYBOARD_ENHANCED: AtomicBool = AtomicBool::new(false);

/// Put the terminal back the way it was found.
///
/// Both the guard's `Drop` and the panic hook call this, and the first one
/// through does the work, so a panic during a normal teardown cannot send the
/// escape sequences twice.
fn restore_terminal() {
    if !TERMINAL_ENTERED.swap(false, Ordering::SeqCst) {
        return;
    }
    drop(disable_raw_mode());
    let mut stdout = io::stdout();
    if KEYBOARD_ENHANCED.swap(false, Ordering::SeqCst) {
        drop(execute!(stdout, PopKeyboardEnhancementFlags));
    }
    drop(execute!(
        stdout,
        DisableMouseCapture,
        DisableBracketedPaste,
        LeaveAlternateScreen,
        Show
    ));
}

/// Restore the terminal before anything reports a panic.
///
/// A release build aborts rather than unwinds, so `Drop` never runs and the
/// hook is the only chance to leave the alternate screen. Without it a panic
/// from any dependency hands the reader a shell still in raw mode, with the
/// message painted on a screen that is about to disappear.
fn install_panic_hook() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        report(info);
    }));
}

fn open_terminal(options: &TerminalOptions) -> Result<()> {
    if !io::stdin().is_terminal() || !cli::stdout_is_terminal() {
        anyhow::bail!("Quinjet requires an interactive terminal");
    }

    install_panic_hook();
    let repository = Repository::discover(&options.path)?;
    let worker = GitWorker::start(repository.clone());
    let watcher = RepoWatcher::new(repository.root()).ok();
    let webhooks = options
        .webhook_listen
        .as_deref()
        .map(WebhookListener::bind)
        .transpose()?;
    let mut app = App::new(repository.root(), repository.name());
    let mut terminal = TerminalGuard::enter(!options.no_mouse)?;
    let render_tick = tick(Duration::from_millis(16));
    let periodic_refresh = tick(Duration::from_secs(10));
    let mut dirty = true;
    let mut running = true;

    app.mouse_capture = !options.no_mouse;
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
            AppEffect::OpenUrl(url) => {
                drop(cli::open_url(&url));
            }
            AppEffect::Quit => running = false,
        }
    }
    running
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    mouse: bool,
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
        TERMINAL_ENTERED.store(true, Ordering::SeqCst);
        if crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false) {
            drop(execute!(
                stdout,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                )
            ));
            KEYBOARD_ENHANCED.store(true, Ordering::SeqCst);
        }
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("failed to initialize terminal")?;
        terminal.clear().context("failed to clear terminal")?;
        Ok(Self { terminal, mouse })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}
